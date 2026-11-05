//! The vault index: every note's headings, `[[links]]` and full text in one
//! SQLite file (`ito-store`, FTS5 trigram, so Japanese is searchable). It is
//! UI-free and never runs on the typing path: `sync` is meant for a worker,
//! and re-reads only the files whose time or size changed.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use ito_store::rusqlite::{self, params};
use ito_store::Store;

use crate::vault;

const SCHEMA: &str = "
CREATE TABLE notes (id INTEGER PRIMARY KEY, path TEXT NOT NULL UNIQUE, mtime INTEGER NOT NULL, size INTEGER NOT NULL, body TEXT NOT NULL);
CREATE TABLE headings (note INTEGER NOT NULL REFERENCES notes(id) ON DELETE CASCADE, line INTEGER NOT NULL, level INTEGER NOT NULL, title TEXT NOT NULL);
CREATE INDEX headings_note ON headings(note);
CREATE TABLE links (note INTEGER NOT NULL REFERENCES notes(id) ON DELETE CASCADE, target TEXT NOT NULL, heading TEXT, line INTEGER NOT NULL);
CREATE INDEX links_note ON links(note);
CREATE INDEX links_target ON links(target COLLATE NOCASE);";

/// Where the index of the vault at `root` lives: the app's state folder, one
/// file per vault (never inside the user's folder).
pub fn path_for(root: &Path) -> Option<PathBuf> {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in root.to_string_lossy().to_lowercase().replace('\\', "/").bytes() {
        h = (h ^ u64::from(b)).wrapping_mul(0x100000001b3);
    }
    ito_common::state_dir("weft").map(|d| d.join(format!("index-{h:016x}.sqlite3")))
}

/// Open (creating) the index at `path`.
pub fn open(path: &Path) -> Result<Store, String> {
    let fts = ito_store::fts_sql("notes", "id", &["body"])?;
    Store::open_at(path, &[SCHEMA, &fts])
}

/// What a `sync` did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
    pub total: usize,
}

struct Parsed {
    path: String,
    mtime: i64,
    size: i64,
    body: String,
    headings: Vec<(usize, u8, String)>,
    links: Vec<(String, Option<String>, usize)>,
}

fn stamp(path: &Path) -> Option<(i64, i64)> {
    let m = std::fs::metadata(path).ok()?;
    let t = m.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some((t.as_nanos() as i64, m.len() as i64))
}

fn parse(root: &Path, rel: &str, (mtime, size): (i64, i64)) -> Option<Parsed> {
    let bytes = std::fs::read(root.join(rel)).ok()?;
    let body = String::from_utf8_lossy(&bytes).into_owned();
    let headings = ito_outline::headings(&body, 6).into_iter().map(|h| (h.line, h.level, h.title)).collect();
    let starts: Vec<usize> = std::iter::once(0).chain(body.match_indices('\n').map(|(i, _)| i + 1)).collect();
    let links = vault::wikilinks(&body)
        .into_iter()
        .map(|l| {
            let line = starts.partition_point(|&s| s <= l.range.start) - 1;
            (l.target, l.heading, line)
        })
        .collect();
    Some(Parsed { path: rel.to_string(), mtime, size, body, headings, links })
}

/// Bring the index in step with the files under `root`: read only the notes
/// that are new or whose time or size changed, and drop the ones that are gone.
pub fn sync(store: &Store, root: &Path) -> Result<Stats, String> {
    let known: HashMap<String, (i64, i64)> = store.read(|c| {
        let mut st = c.prepare("SELECT path, mtime, size FROM notes")?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, (r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))))?;
        rows.collect()
    })?;
    let on_disk = vault::scan(root);
    let mut stats = Stats { total: on_disk.len(), ..Stats::default() };
    let mut changed = Vec::new();
    for rel in &on_disk {
        let Some(now) = stamp(&root.join(rel)) else { continue };
        match known.get(rel) {
            Some(was) if *was == now => {}
            Some(_) => {
                stats.updated += 1;
                changed.extend(parse(root, rel, now));
            }
            None => {
                stats.added += 1;
                changed.extend(parse(root, rel, now));
            }
        }
    }
    let present: std::collections::HashSet<&String> = on_disk.iter().collect();
    let gone: Vec<String> = known.keys().filter(|k| !present.contains(k)).cloned().collect();
    stats.removed = gone.len();
    if changed.is_empty() && gone.is_empty() {
        return Ok(stats);
    }
    store
        .write(move |t| {
            for p in &gone {
                t.execute("DELETE FROM notes WHERE path = ?1", [p])?;
            }
            for n in &changed {
                t.execute("DELETE FROM notes WHERE path = ?1", [&n.path])?;
                t.execute("INSERT INTO notes (path, mtime, size, body) VALUES (?1, ?2, ?3, ?4)", params![n.path, n.mtime, n.size, n.body])?;
                let id = t.last_insert_rowid();
                for (line, level, title) in &n.headings {
                    t.execute(
                        "INSERT INTO headings (note, line, level, title) VALUES (?1, ?2, ?3, ?4)",
                        params![id, *line as i64, *level as i64, title],
                    )?;
                }
                for (target, heading, line) in &n.links {
                    t.execute(
                        "INSERT INTO links (note, target, heading, line) VALUES (?1, ?2, ?3, ?4)",
                        params![id, target, heading, *line as i64],
                    )?;
                }
            }
            Ok(())
        })
        .wait()?;
    Ok(stats)
}

/// A place a note is linked from.
#[derive(Debug, PartialEq, Eq)]
pub struct Backlink {
    pub path: String,
    /// 0-based line of the `[[link]]`.
    pub line: usize,
}

/// The notes (and lines) that link to `note` (a vault-relative path). A bare
/// name counts only when it resolves to `note`, as a click on it would.
pub fn backlinks(store: &Store, note: &str) -> Result<Vec<Backlink>, String> {
    let no_ext = note.trim_end_matches(".md").to_lowercase();
    let stem = no_ext.rsplit('/').next().unwrap_or(&no_ext).to_string();
    store.read(|c| {
        let notes: Vec<String> = c.prepare("SELECT path FROM notes")?.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        let mut st = c.prepare(
            "SELECT n.path, l.line, l.target FROM links l JOIN notes n ON n.id = l.note \
             WHERE l.target COLLATE NOCASE IN (?1, ?2, ?3) ORDER BY n.path, l.line",
        )?;
        let rows = st.query_map(params![stem, no_ext, format!("{no_ext}.md")], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (path, line, target) = row?;
            if path != note && vault::resolve(&notes, &target) == Some(note) {
                out.push(Backlink { path, line: line as usize });
            }
        }
        Ok(out)
    })
}

/// The notes whose text holds every word of `query`, best first. Words of
/// three or more characters use the full-text table; shorter ones filter with
/// `LIKE` (the trigram tokenizer cannot hold them).
pub fn search(store: &Store, query: &str, limit: usize) -> Result<Vec<String>, String> {
    let q = ito_store::parse_query(query);
    if q.fts.is_none() && q.short.is_empty() {
        return Ok(Vec::new());
    }
    store.read(|c| {
        let mut ids = match &q.fts {
            Some(_) => ito_store::search_ids(c, "notes", &q, 500)?,
            None => c.prepare("SELECT id FROM notes ORDER BY path")?.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?,
        };
        let mut out = Vec::new();
        for id in ids.drain(..) {
            let (path, body): (String, String) =
                c.query_row("SELECT path, body FROM notes WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))?;
            let lower = body.to_lowercase();
            if q.short.iter().all(|w| lower.contains(&w.to_lowercase())) {
                out.push(path);
                if out.len() == limit {
                    break;
                }
            }
        }
        Ok(out)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("weft-index-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn sync_reads_only_what_changed_and_finds_links_and_text() {
        let root = tmp("a");
        std::fs::write(root.join("a.md"), "# Alpha\n\nsee [[b]]\n").unwrap();
        std::fs::create_dir(root.join("sub")).unwrap();
        std::fs::write(root.join("sub/b.md"), "# 買い物リスト\n\n牛乳とパン\nback [[a#Alpha|x]] and [[B]]\n").unwrap();
        let store = open(&root.join("i.sqlite3")).unwrap();

        assert_eq!(sync(&store, &root).unwrap(), Stats { added: 2, updated: 0, removed: 0, total: 2 });
        assert_eq!(sync(&store, &root).unwrap(), Stats { added: 0, updated: 0, removed: 0, total: 2 });

        assert_eq!(backlinks(&store, "sub/b.md").unwrap(), vec![Backlink { path: "a.md".into(), line: 2 }]);
        assert_eq!(backlinks(&store, "a.md").unwrap(), vec![Backlink { path: "sub/b.md".into(), line: 3 }]);
        assert_eq!(search(&store, "牛乳とパン", 10).unwrap(), vec!["sub/b.md".to_string()]);
        assert_eq!(search(&store, "パン", 10).unwrap(), vec!["sub/b.md".to_string()]);
        assert!(search(&store, "存在しない言葉", 10).unwrap().is_empty());

        std::fs::write(root.join("a.md"), "# Alpha\n\nno link now, longer text\n").unwrap();
        std::fs::remove_file(root.join("sub/b.md")).unwrap();
        let s = sync(&store, &root).unwrap();
        assert_eq!((s.updated, s.removed, s.total), (1, 1, 1));
        assert!(backlinks(&store, "sub/b.md").unwrap().is_empty());
        assert!(search(&store, "牛乳とパン", 10).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// `cargo test -p weft --release index_ten_thousand -- --ignored --nocapture`
    #[test]
    #[ignore = "a measurement, not a check"]
    fn index_ten_thousand_notes() {
        let root = tmp("big");
        for d in 0..100 {
            let dir = root.join(format!("d{d}"));
            std::fs::create_dir(&dir).unwrap();
            for n in 0..100 {
                let body = format!(
                    "# Note {d}-{n}

本文 {d} {n} "
                ) + &"あいうえお lorem ipsum dolor sit amet ".repeat(40)
                    + &format!(
                        "

## 節
[[d{}/n{}]]
",
                        (d + 1) % 100,
                        n
                    );
                std::fs::write(dir.join(format!("n{n}.md")), body).unwrap();
            }
        }
        let store = open(&root.join("i.sqlite3")).unwrap();
        let t = std::time::Instant::now();
        let s = sync(&store, &root).unwrap();
        println!("first sync: {:?} ({} notes)", t.elapsed(), s.total);
        let t = std::time::Instant::now();
        sync(&store, &root).unwrap();
        println!("sync, nothing changed: {:?}", t.elapsed());
        let t = std::time::Instant::now();
        let hits = search(&store, "本文 7 7 あいうえお", 20).unwrap();
        println!("search: {:?} ({} hits)", t.elapsed(), hits.len());
        let _ = std::fs::remove_dir_all(&root);
    }
}
