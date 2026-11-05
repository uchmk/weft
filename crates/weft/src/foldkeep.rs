//! Folded headings kept between runs: `folds.toml` in the state folder holds,
//! for the files last folded, which headings were folded. A file that is
//! opened again gets them back (each finds its heading by place or title, as
//! the folds do when the text changes). Written on the writer's thread.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::fold::Key;
use crate::App;

/// How many files are remembered.
const KEEP: usize = 200;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct Entry {
    pub path: PathBuf,
    pub folds: Vec<Key>,
}

#[derive(Serialize, Deserialize, Default)]
struct File {
    #[serde(default)]
    entries: Vec<Entry>,
}

pub(crate) fn state_path() -> Option<PathBuf> {
    ito_common::state_dir("weft").map(|d| d.join("folds.toml"))
}

pub(crate) fn read() -> Vec<Entry> {
    state_path().and_then(|p| ito_common::kept::read::<File>(&p)).map(|f| f.entries).unwrap_or_default()
}

/// Record `folds` for `path` (oldest first, the newest last); true if the list changed.
fn record(store: &mut Vec<Entry>, path: PathBuf, folds: &[Key]) -> bool {
    let at = store.iter().position(|e| e.path == path);
    match (at, folds.is_empty()) {
        (None, true) => false,
        (Some(i), true) => {
            store.remove(i);
            true
        }
        (Some(i), false) if store[i].folds == folds => false,
        (found, false) => {
            if let Some(i) = found {
                store.remove(i);
            }
            store.push(Entry { path, folds: folds.to_vec() });
            if store.len() > KEEP {
                store.remove(0);
            }
            true
        }
    }
}

impl App {
    /// The folds kept for `path`.
    pub(crate) fn kept_folds(&self, path: &std::path::Path) -> Vec<Key> {
        self.fold_store.iter().find(|e| e.path == path).map(|e| e.folds.clone()).unwrap_or_default()
    }

    /// One frame: keep the folds of the open documents when they changed.
    pub(crate) fn take_folds(&mut self) {
        let now: Vec<(PathBuf, Vec<Key>)> = self.docs().filter_map(|d| Some((d.path()?, d.view.folds.keys().to_vec()))).collect();
        let mut changed = false;
        for (path, keys) in now {
            changed |= record(&mut self.fold_store, path, &keys);
        }
        if changed {
            self.fold_writer.save_toml(&File { entries: self.fold_store.clone() });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(i: usize) -> Key {
        toml::from_str::<Entry>(&format!("path = \"a\"\n[[folds]]\nindex = {i}\nlevel = 1\ntitle = \"t\"\n")).unwrap().folds.remove(0)
    }

    #[test]
    fn folds_are_recorded_replaced_and_dropped() {
        let mut s = Vec::new();
        let a = PathBuf::from("a");
        assert!(!record(&mut s, a.clone(), &[]));
        assert!(record(&mut s, a.clone(), &[k(1)]));
        assert!(!record(&mut s, a.clone(), &[k(1)]));
        assert!(record(&mut s, a.clone(), &[k(2)]));
        assert_eq!(s.len(), 1);
        assert!(record(&mut s, a, &[]));
        assert!(s.is_empty());
    }
}
