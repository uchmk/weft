//! The vault: the folder of Markdown files a document belongs to. This is the
//! UI-free base of phase 5: where the vault is, which notes it holds, which
//! `[[links]]` a text has, and which notes a typed prefix suggests. The
//! completion popup itself waits for an inline popup in ito (uchmk/ito#198).

use std::path::{Path, PathBuf};

/// Folders a scan never enters: version control, snapshots, build output.
const SKIP_DIRS: &[&str] = &[".git", ".weft", ".snapshots", "node_modules", "target"];

/// The vault of `file`: the nearest ancestor holding a `.weft` or `.git`
/// folder, else the file's own folder.
pub fn root_of(file: &Path) -> PathBuf {
    let start = file.parent().unwrap_or(file);
    start.ancestors().find(|d| d.join(".weft").is_dir() || d.join(".git").exists()).unwrap_or(start).to_path_buf()
}

/// Every `.md` file under `root` (relative paths, `/` separators, sorted).
pub fn scan(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let path = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        let Ok(ty) = e.file_type() else { continue };
        if ty.is_dir() {
            if !SKIP_DIRS.contains(&name.as_str()) {
                walk(root, &path, out);
            }
        } else if name.to_ascii_lowercase().ends_with(".md") {
            if let Ok(rel) = path.strip_prefix(root) {
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
}

/// One `[[target|label]]` or `[[target#heading]]` in a text.
#[derive(Debug, PartialEq, Eq)]
pub struct WikiLink {
    /// Byte range of the whole `[[…]]`.
    pub range: std::ops::Range<usize>,
    pub target: String,
    pub heading: Option<String>,
    pub label: Option<String>,
}

/// The wiki links of `text`, skipping fenced code blocks and code spans.
pub fn wikilinks(text: &str) -> Vec<WikiLink> {
    let mut out = Vec::new();
    let mut fenced = false;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            fenced = !fenced;
        } else if !fenced {
            links_in_line(line, offset, &mut out);
        }
        offset += line.len();
    }
    out
}

fn links_in_line(line: &str, base: usize, out: &mut Vec<WikiLink>) {
    let b = line.as_bytes();
    let (mut i, mut in_code) = (0, false);
    while i < b.len() {
        match b[i] {
            b'`' => {
                in_code = !in_code;
                i += 1;
            }
            b'[' if !in_code && b.get(i + 1) == Some(&b'[') => {
                let Some(len) = line[i + 2..].find("]]") else { break };
                let inner = &line[i + 2..i + 2 + len];
                if !inner.is_empty() && !inner.contains(['[', '\n']) {
                    out.push(parse(inner, base + i..base + i + len + 4));
                }
                i += len + 4;
            }
            _ => i += 1,
        }
    }
}

fn parse(inner: &str, range: std::ops::Range<usize>) -> WikiLink {
    let (head, label) = match inner.split_once('|') {
        Some((h, l)) => (h, Some(l.trim().to_string())),
        None => (inner, None),
    };
    let (target, heading) = match head.split_once('#') {
        Some((t, h)) => (t, Some(h.trim().to_string())),
        None => (head, None),
    };
    WikiLink { range, target: target.trim().to_string(), heading, label }
}

/// The note a link target names: an exact path or file name (case-insensitive,
/// `.md` optional). A bare name matching several notes picks the shortest path.
pub fn resolve<'a>(notes: &'a [String], target: &str) -> Option<&'a str> {
    let want = target.trim().trim_end_matches(".md").to_lowercase();
    notes
        .iter()
        .filter(|n| {
            let n = n.trim_end_matches(".md").to_lowercase();
            n == want || n.rsplit('/').next() == Some(want.as_str())
        })
        .min_by_key(|n| n.len())
        .map(String::as_str)
}

/// Notes matching a typed `query`, best first (the name counts, not the folder).
pub fn suggest<'a>(notes: &'a [String], query: &str, limit: usize) -> Vec<&'a str> {
    let sensitive = query.chars().any(char::is_uppercase);
    let mut hits: Vec<(i32, &str)> = notes
        .iter()
        .filter_map(|n| {
            let stem = n.rsplit('/').next().unwrap_or(n).trim_end_matches(".md");
            let h = ito_match::fuzzy::match_str(query, stem, sensitive)?;
            Some((h.score, n.as_str()))
        })
        .collect();
    hits.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.len().cmp(&b.1.len())).then(a.1.cmp(b.1)));
    hits.into_iter().take(limit).map(|(_, n)| n).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_have_target_heading_and_label_and_skip_code() {
        let t = "a [[Note]] b [[Other#Intro|see]]\n`[[no]]` [[x\n```\n[[fenced]]\n```\n[[Last]]";
        let l = wikilinks(t);
        let names: Vec<_> = l.iter().map(|w| w.target.as_str()).collect();
        assert_eq!(names, ["Note", "Other", "Last"]);
        assert_eq!(l[1].heading.as_deref(), Some("Intro"));
        assert_eq!(l[1].label.as_deref(), Some("see"));
        assert_eq!(&t[l[0].range.clone()], "[[Note]]");
    }

    #[test]
    fn resolve_ignores_case_extension_and_folder() {
        let n = vec!["a/Note.md".to_string(), "deep/er/note.md".to_string()];
        assert_eq!(resolve(&n, "note"), Some("a/Note.md"));
        assert_eq!(resolve(&n, "deep/er/note.md"), Some("deep/er/note.md"));
        assert_eq!(resolve(&n, "missing"), None);
    }

    #[test]
    fn suggest_ranks_the_name_and_japanese_works() {
        let n: Vec<String> = ["journal/2026.md", "ideas.md", "メモ/買い物.md"].iter().map(|s| s.to_string()).collect();
        assert_eq!(suggest(&n, "ide", 5), ["ideas.md"]);
        assert_eq!(suggest(&n, "買", 5), ["メモ/買い物.md"]);
    }

    #[test]
    fn scan_finds_notes_and_skips_hidden_folders() {
        let d = std::env::temp_dir().join(format!("weft-vault-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("sub")).unwrap();
        std::fs::create_dir_all(d.join(".snapshots")).unwrap();
        std::fs::create_dir_all(d.join(".weft")).unwrap();
        for f in ["a.md", "sub/b.MD", ".snapshots/old.md", "c.txt"] {
            std::fs::write(d.join(f), "x").unwrap();
        }
        assert_eq!(scan(&d), ["a.md", "sub/b.MD"]);
        assert_eq!(root_of(&d.join("sub").join("b.MD")), d);
        let _ = std::fs::remove_dir_all(&d);
    }
}
