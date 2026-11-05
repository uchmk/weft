//! What changed since the file was read or saved, for the gutter: a bar by
//! each added or changed line and a mark where lines were taken out.
//!
//! The comparison is `ito-diff`'s, run on a thread (never on the input path);
//! only the lines between the first and the last difference are compared, so
//! an edit in a large file costs what the edit spans. A result belongs to a
//! version of the text and of the baseline; a newer one replaces it.

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

use ito_diff::Kind;

/// What the gutter says about a line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    /// A line that is not in the baseline.
    Added,
    /// A line that replaces lines of the baseline.
    Modified,
    /// Lines of the baseline were taken out just above this one.
    Removed,
}

/// The marks of a text against its baseline, one per line of the text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Changes {
    /// The text and baseline versions it was computed for.
    pub key: (u64, u64),
    pub marks: Vec<Option<Mark>>,
}

/// The marks of `text` against `base`.
pub fn compute(base: &str, text: &str) -> Vec<Option<Mark>> {
    let old: Vec<&str> = base.split_inclusive('\n').collect();
    let new: Vec<&str> = text.split_inclusive('\n').collect();
    let mut marks = vec![None; new.len()];
    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let suffix = old[prefix..].iter().rev().zip(new[prefix..].iter().rev()).take_while(|(a, b)| a == b).count();
    let (old_mid, new_mid) = (&old[prefix..old.len() - suffix], &new[prefix..new.len() - suffix]);
    if old_mid.is_empty() && new_mid.is_empty() {
        return marks;
    }
    let lines = ito_diff::diff_lines(&old_mid.concat(), &new_mid.concat());
    let mut gone = 0usize;
    // The lines taken out come before the line that follows them.
    let last = marks.len().saturating_sub(1);
    let at = |i: usize| i.min(last);
    let mut next = prefix;
    for l in &lines {
        match l.kind {
            Kind::Removed => gone += 1,
            Kind::Added => {
                let i = prefix + l.new_no.unwrap_or(1) - 1;
                if gone > 0 {
                    gone -= 1;
                    marks[i] = Some(Mark::Modified);
                } else {
                    marks[i] = Some(Mark::Added);
                }
                next = i + 1;
            }
            Kind::Same => {
                let i = prefix + l.new_no.unwrap_or(1) - 1;
                if gone > 0 && !marks.is_empty() {
                    marks[at(i)] = marks[at(i)].or(Some(Mark::Removed));
                }
                gone = 0;
                next = i + 1;
            }
        }
    }
    if gone > 0 && !marks.is_empty() {
        let i = at(next);
        marks[i] = marks[i].or(Some(Mark::Removed));
    }
    marks
}

struct Job {
    key: (u64, u64),
    base: Arc<String>,
    text: String,
}

/// The comparison of one document: the thread and the newest result.
pub struct Worker {
    tx: Sender<Job>,
    rx: Receiver<Changes>,
    asked: Option<(u64, u64)>,
    newest: Changes,
}

impl Worker {
    /// `wake` is called when a result is ready (the window's repaint).
    pub fn new(wake: impl Fn() + Send + 'static) -> Worker {
        let (tx, jobs) = mpsc::channel::<Job>();
        let (done, rx) = mpsc::channel();
        let run = move || {
            while let Ok(mut job) = jobs.recv() {
                while let Ok(newer) = jobs.try_recv() {
                    job = newer;
                }
                let marks = compute(&job.base, &job.text);
                if done.send(Changes { key: job.key, marks }).is_err() {
                    break;
                }
                wake();
            }
        };
        let _ = std::thread::Builder::new().name("weft-changes".into()).spawn(run);
        Worker { tx, rx, asked: None, newest: Changes::default() }
    }

    /// Ask for the marks of `text` (at `version`) against `base` (`generation`
    /// says which baseline) unless that was asked already, and keep the newest
    /// result that came back.
    pub fn update(&mut self, version: u64, generation: u64, base: &Arc<String>, text: &str) -> &Changes {
        let key = (version, generation);
        if self.asked != Some(key) {
            self.asked = Some(key);
            let _ = self.tx.send(Job { key, base: base.clone(), text: text.to_owned() });
        }
        while let Ok(c) = self.rx.try_recv() {
            if c.key >= self.newest.key {
                self.newest = c;
            }
        }
        &self.newest
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Mark::*;

    #[test]
    fn equal_texts_have_no_marks() {
        assert_eq!(compute("a\nb\n", "a\nb\n"), [None, None]);
        assert!(compute("", "").is_empty());
    }

    #[test]
    fn added_modified_and_removed_lines_are_told_apart() {
        assert_eq!(compute("a\nb\nc\n", "a\nb\nc\nd\n"), [None, None, None, Some(Added)]);
        assert_eq!(compute("a\nb\nc\n", "a\nB\nc\n"), [None, Some(Modified), None]);
        assert_eq!(compute("a\nb\nc\n", "a\nc\n"), [None, Some(Removed)], "b is gone: the mark is on the line after it");
        assert_eq!(compute("a\nb\n", "a\n"), [Some(Removed)], "the last lines taken out mark the last line left");
        assert_eq!(compute("a\nb\nc\n", "a\nB\nB2\nc\n"), [None, Some(Modified), Some(Added), None]);
        assert_eq!(compute("", "x\ny"), [Some(Added), Some(Added)]);
    }

    #[test]
    fn the_worker_answers_for_the_newest_text() {
        let mut w = Worker::new(|| {});
        let base = Arc::new("a\nb\n".to_owned());
        w.update(1, 0, &base, "a\nB\n");
        let mut got = Changes::default();
        for _ in 0..200 {
            got = w.update(1, 0, &base, "a\nB\n").clone();
            if got.key == (1, 0) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(got.marks, [None, Some(Modified)]);
    }
}
