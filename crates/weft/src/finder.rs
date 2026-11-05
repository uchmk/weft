//! The vault's two lists over the index: full-text search (`vault.search`)
//! and the notes that link here (`vault.backlinks`). Both are the palette box
//! with its own matching; the index is brought up to date on a worker, and
//! the box says so until it is ready.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

use ito_store::Store;

use crate::index;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Search,
    Backlinks,
}

/// What a line of the list opens: the note and the 0-based line to land on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    pub path: String,
    pub line: Option<usize>,
}

/// One open list.
pub struct Finder {
    pub kind: Kind,
    pub root: PathBuf,
    /// The note the backlinks are of (vault-relative); empty for a search.
    note: String,
    store: Arc<Store>,
    ready: Option<Receiver<Result<index::Stats, String>>>,
    pub is_ready: bool,
    pub error: Option<String>,
    shown_for: Option<String>,
    pub entries: Vec<ito_palette::Entry<Hit>>,
}

impl Finder {
    /// Open the index of the vault at `root` and start bringing it up to date.
    pub fn open(kind: Kind, root: PathBuf, note: String) -> Result<Finder, String> {
        let path = index::path_for(&root).ok_or("no state folder")?;
        let store = Arc::new(index::open(&path)?);
        let (tx, rx) = channel();
        let (s, r) = (store.clone(), root.clone());
        std::thread::Builder::new()
            .name("weft-index".into())
            .spawn(move || {
                let _ = tx.send(index::sync(&s, &r));
            })
            .map_err(|e| e.to_string())?;
        Ok(Finder { kind, root, note, store, ready: Some(rx), is_ready: false, error: None, shown_for: None, entries: Vec::new() })
    }

    /// Take the worker's answer when it has come; refresh the list for `query`
    /// when it or the index changed. True while the worker still runs.
    pub fn update(&mut self, query: &str) -> bool {
        if let Some(rx) = &self.ready {
            match rx.try_recv() {
                Ok(done) => {
                    self.ready = None;
                    self.is_ready = true;
                    self.shown_for = None;
                    if let Err(e) = done {
                        self.error = Some(e);
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => return true,
                Err(_) => {
                    self.ready = None;
                    self.error = Some("the index worker stopped".into());
                }
            }
        }
        if self.is_ready && self.shown_for.as_deref() != Some(query) {
            self.shown_for = Some(query.to_string());
            self.entries = self.list(query);
        }
        false
    }

    fn list(&self, query: &str) -> Vec<ito_palette::Entry<Hit>> {
        match self.kind {
            Kind::Search => {
                let paths = index::search(&self.store, query, 200).unwrap_or_default();
                let first = query.split_whitespace().next().unwrap_or("");
                paths
                    .into_iter()
                    .map(|p| {
                        let line = line_of(&self.root.join(&p), first);
                        let mut e = ito_palette::Entry::new(p.clone(), "", Hit { path: p, line });
                        e.detail = line.and_then(|l| line_text(&self.root.join(&e.pick.path), l)).unwrap_or_default();
                        e
                    })
                    .collect()
            }
            Kind::Backlinks => {
                let all = index::backlinks(&self.store, &self.note).unwrap_or_default();
                all.into_iter()
                    .filter(|b| query.is_empty() || b.path.to_lowercase().contains(&query.to_lowercase()))
                    .map(|b| {
                        let mut e = ito_palette::Entry::new(b.path.clone(), "", Hit { path: b.path.clone(), line: Some(b.line) });
                        e.detail = line_text(&self.root.join(&b.path), b.line).unwrap_or_default();
                        e
                    })
                    .collect()
            }
        }
    }
}

/// The 0-based line of the first place `word` is in the file (case aside).
fn line_of(file: &Path, word: &str) -> Option<usize> {
    if word.is_empty() {
        return None;
    }
    let w = word.to_lowercase();
    let text = std::fs::read_to_string(file).ok()?;
    text.lines().position(|l| l.to_lowercase().contains(&w))
}

fn line_text(file: &Path, line: usize) -> Option<String> {
    let text = std::fs::read_to_string(file).ok()?;
    Some(text.lines().nth(line)?.trim().chars().take(120).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_waits_for_the_index_then_follows_the_query() {
        let root = std::env::temp_dir().join(format!("weft-finder-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(".weft")).unwrap();
        std::fs::write(root.join("a.md"), "# A\n\nsee [[b]]\n").unwrap();
        std::fs::write(root.join("b.md"), "# B\n\n牛乳とパン\n").unwrap();
        let mut f = Finder::open(Kind::Backlinks, root.clone(), "b.md".into()).unwrap();
        let mut spins = 0;
        while f.update("") {
            spins += 1;
            assert!(spins < 500, "the worker never answered");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(f.entries.len(), 1);
        assert_eq!(f.entries[0].pick, Hit { path: "a.md".into(), line: Some(2) });
        assert_eq!(f.entries[0].detail, "see [[b]]");

        let mut s = Finder::open(Kind::Search, root.clone(), String::new()).unwrap();
        while s.update("牛乳とパン") {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(s.entries[0].pick, Hit { path: "b.md".into(), line: Some(2) });
        drop((f, s));
        let _ = std::fs::remove_dir_all(&root);
    }
}
