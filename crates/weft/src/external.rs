//! Changes made to an open file by someone else (another editor, a sync
//! tool, git): the files' folders are watched, and what the disk holds is
//! read on the file thread and compared with what we last read or wrote.
//!
//! A document with no unsaved changes simply follows the file (as one undo
//! step). One with unsaved changes asks: what the disk holds goes in (and
//! Ctrl+Z brings the window's own text back), or the window's text stays and
//! the next save writes over the file.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::{App, Doc};

/// How long a burst of events waits before the file is read (an editor's
/// save is several events).
const SETTLE: Duration = Duration::from_millis(300);

/// How many changed lines the question shows.
const SHOWN: usize = 6;

/// A file the question is about: what the disk holds now.
pub(crate) struct Asked {
    pub path: PathBuf,
    pub disk: String,
}

/// The lines that differ between `mine` and `disk` (the part between the
/// first and the last difference), for the question: how many each side
/// has, and a few of them marked `+` (the disk's) and `-` (the window's).
pub(crate) fn summary(mine: &str, disk: &str) -> (usize, usize, Vec<String>) {
    let disk = disk.strip_prefix('\u{feff}').unwrap_or(disk);
    let a: Vec<&str> = mine.split_inclusive('\n').collect();
    let b: Vec<&str> = disk.split_inclusive('\n').collect();
    let head = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let tail = a[head..].iter().rev().zip(b[head..].iter().rev()).take_while(|(x, y)| x == y).count();
    let (am, bm) = (&a[head..a.len() - tail], &b[head..b.len() - tail]);
    // A very long difference is counted, not listed.
    if am.len() + bm.len() > 4000 {
        return (bm.len(), am.len(), Vec::new());
    }
    let (old, new) = (am.concat(), bm.concat());
    let mut added = 0;
    let mut removed = 0;
    let mut shown = Vec::new();
    for l in ito_diff::diff_lines(&old, &new) {
        let mark = match l.kind {
            ito_diff::Kind::Same => continue,
            ito_diff::Kind::Added => {
                added += 1;
                '+'
            }
            ito_diff::Kind::Removed => {
                removed += 1;
                '-'
            }
        };
        if shown.len() < SHOWN {
            let text: String = l.text.trim_end_matches(['\r', '\n']).chars().take(60).collect();
            shown.push(format!("{mark} {text}"));
        }
    }
    (added, removed, shown)
}

impl App {
    /// Every folder with an open file, once.
    fn watched_dirs(&self) -> Vec<PathBuf> {
        let mut dirs: Vec<PathBuf> = self.docs().filter_map(Doc::path).filter_map(|p| p.parent().map(Path::to_path_buf)).collect();
        dirs.sort();
        dirs.dedup();
        dirs
    }

    /// One frame: keep the watched folders up to date, turn the watcher's
    /// events into reads (after they settle), and put the answers to them in.
    pub(crate) fn take_external(&mut self, ctx: &egui::Context) {
        let dirs = self.watched_dirs();
        self.dirwatch.sync(dirs.iter().map(PathBuf::as_path));
        while let Ok(dir) = self.dirwatch.rx.try_recv() {
            let hit: Vec<PathBuf> = self.docs().filter_map(Doc::path).filter(|p| p.parent() == Some(dir.as_path())).collect();
            for p in hit {
                if !self.watch_wait.iter().any(|(w, _)| *w == p) {
                    self.watch_wait.push((p, Instant::now()));
                }
            }
        }
        if self.watch_wait.is_empty() {
            return;
        }
        let now = Instant::now();
        let (ready, waiting): (Vec<_>, Vec<_>) =
            std::mem::take(&mut self.watch_wait).into_iter().partition(|(_, at)| now.duration_since(*at) >= SETTLE);
        self.watch_wait = waiting;
        if !self.watch_wait.is_empty() {
            ctx.request_repaint_after(SETTLE);
        }
        for (path, _) in ready {
            // A save in flight will land with the same text; look after it.
            if self.doc_ref(&path).is_some_and(|d| d.saving > 0) {
                self.watch_wait.push((path, now));
                ctx.request_repaint_after(SETTLE);
                continue;
            }
            self.send_file(crate::FileJob::Check { path });
        }
    }

    /// What the file thread read of `path`: `None` when it is gone or not
    /// UTF-8 (the window keeps what it has).
    pub(crate) fn disk_read(&mut self, path: PathBuf, raw: Option<String>) {
        let Some(raw) = raw else { return };
        let Some(d) = self.doc_mut(&path) else { return };
        let Some(ed) = d.editor.as_mut() else { return };
        if raw == ed.disk() || d.declined.as_deref() == Some(raw.as_str()) {
            return;
        }
        d.declined = None;
        let short = path.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned());
        if !ed.is_dirty() {
            ed.reload(&raw, true);
            let note = self.table.trf("external.reloaded", &[&short]);
            self.announce(note.clone());
            self.note = Some(note);
        } else if ed.text() == raw.strip_prefix('\u{feff}').unwrap_or(&raw) {
            // What was typed is what the file now holds.
            ed.reload(&raw, true);
        } else if let Some(a) = self.asked.iter_mut().find(|a| a.path == path) {
            a.disk = raw;
        } else {
            self.asked.push(Asked { path, disk: raw });
        }
    }

    /// The question's answer: 0 keep the window's text, 1 take the disk's.
    pub(crate) fn answer_external(&mut self, pick: usize) {
        if self.asked.is_empty() {
            return;
        }
        let Asked { path, disk } = self.asked.remove(0);
        let Some(d) = self.doc_mut(&path) else { return };
        if pick == 0 {
            d.declined = Some(disk);
        } else if let Some(ed) = d.editor.as_mut() {
            // One undo step: Ctrl+Z brings back what the window had. It stays
            // changed, so closing still asks and saving writes the window's.
            ed.reload(&disk, false);
        }
    }

    pub(crate) fn external_open(&self) -> bool {
        !self.asked.is_empty() && self.recover.is_empty() && self.quit.is_none() && self.closing.is_none()
    }

    /// The question's keys, read before anything else sees them: Esc keeps
    /// the window's text, Enter (or `y`) takes the file's.
    pub(crate) fn external_keys(&mut self, ctx: &egui::Context) {
        use egui::{Key as K, Modifiers as M};
        if !self.external_open() {
            return;
        }
        let pick = ctx.input_mut(|i| {
            if i.consume_key(M::NONE, K::Escape) {
                Some(0)
            } else if i.consume_key(M::NONE, K::Enter) || i.consume_key(M::NONE, K::Y) {
                Some(1)
            } else {
                None
            }
        });
        if let Some(p) = pick {
            self.answer_external(p);
        }
    }

    /// The question about the file that changed, over everything; a click is its key.
    pub(crate) fn external_question(&mut self, ctx: &egui::Context) {
        if !self.external_open() {
            return;
        }
        let a = &self.asked[0];
        let name = a.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let mine = self.doc_ref(&a.path).and_then(|d| d.editor.as_ref()).map(|e| e.text().to_owned()).unwrap_or_default();
        let (added, removed, shown) = summary(&mine, &a.disk);
        let t = &self.table;
        let mut body = vec![t.trf("external.body", &[&name]), t.trf("external.counts", &[&added.to_string(), &removed.to_string()])];
        body.extend(shown);
        let buttons = [format!("{}  Esc", t.tr("external.keep")), format!("{}  Enter", t.tr("external.load"))];
        let q =
            ito_confirm::Question { title: t.tr("external.title"), body: &body, buttons: &buttons, primary: Some(1), ..Default::default() };
        if let Some(i) = ito_confirm::show(ctx, &q, &crate::confirm_style()) {
            self.answer_external(i);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::summary;

    #[test]
    fn summary_counts_and_marks_only_the_differing_lines() {
        let (added, removed, shown) = summary("a\nb\nc\n", "a\nB\nB2\nc\n");
        assert_eq!((added, removed), (2, 1));
        assert_eq!(shown, ["- b", "+ B", "+ B2"]);
    }

    #[test]
    fn summary_ignores_a_bom_and_caps_the_list() {
        let (a, r, _) = summary("x\n", "\u{feff}x\n");
        assert_eq!((a, r), (0, 0));
        let disk: String = (0..20).map(|i| format!("n{i}\n")).collect();
        let (a, _, shown) = summary("", &disk);
        assert_eq!(a, 20);
        assert_eq!(shown.len(), super::SHOWN);
    }
}
