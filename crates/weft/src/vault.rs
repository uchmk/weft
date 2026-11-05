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
    let mut out = scan_files(root);
    out.retain(|n| is_note(n));
    out
}

/// Every file under `root` (relative paths, `/` separators, sorted).
pub fn scan_files(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

fn is_note(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".md")
}

fn is_image(name: &str) -> bool {
    let l = name.to_ascii_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg", ".bmp", ".avif"].iter().any(|e| l.ends_with(e))
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
        } else if let Ok(rel) = path.strip_prefix(root) {
            out.push(rel.to_string_lossy().replace('\\', "/"));
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

/// The `[[link]]` the caret is in (or touches), if any.
pub fn link_at(text: &str, caret: usize) -> Option<WikiLink> {
    wikilinks(text).into_iter().find(|l| l.range.start <= caret && caret <= l.range.end)
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

/// Where a `[[` completion stands: the byte where the typed name starts, and
/// the name typed so far. `None` when the caret is not inside an open `[[`
/// (a `]]`, `[`, `|` or `#` between them ends it, and so does the line end).
pub fn trigger(text: &str, caret: usize) -> Option<(usize, &str)> {
    let head = text.get(..caret)?;
    let line = head.rfind('\n').map_or(0, |i| i + 1);
    let seg = &head[line..];
    let open = seg.rfind("[[")?;
    let q = &seg[open + 2..];
    if q.contains([']', '[', '|', '#']) {
        return None;
    }
    Some((line + open + 2, q))
}

/// Where a link-path completion stands: the byte where the typed path starts,
/// the path typed so far, and whether the link is an image (`![alt](`). `None`
/// when the caret is not inside an open `](` on its line.
pub fn path_trigger(text: &str, caret: usize) -> Option<(usize, &str, bool)> {
    let head = text.get(..caret)?;
    let line = head.rfind('\n').map_or(0, |i| i + 1);
    let seg = &head[line..];
    let open = seg.rfind("](")?;
    let q = &seg[open + 2..];
    if q.contains([')', ' ', '(', '"']) {
        return None;
    }
    let image = seg[..open].rfind('[').is_some_and(|i| seg[..i].ends_with('!'));
    Some((line + open + 2, q, image))
}

/// The path from the folder `from_dir` (relative to the vault) to `target`
/// (relative to the vault), as a link writes it: `../img/a.png`.
pub fn relative(from_dir: &str, target: &str) -> String {
    let from: Vec<&str> = from_dir.split('/').filter(|p| !p.is_empty()).collect();
    let to: Vec<&str> = target.split('/').collect();
    let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count().min(to.len() - 1);
    let mut out: Vec<&str> = vec![".."; from.len() - common];
    out.extend(&to[common..]);
    out.join("/")
}

/// Files matching a typed path, best first (the whole relative path counts).
pub fn suggest_paths<'a>(files: &'a [String], query: &str, limit: usize) -> Vec<&'a str> {
    let sensitive = query.chars().any(char::is_uppercase);
    let mut hits: Vec<(i32, &str)> =
        files.iter().filter_map(|n| Some((ito_match::fuzzy::match_str(query, n, sensitive)?.score, n.as_str()))).collect();
    hits.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.len().cmp(&b.1.len())).then(a.1.cmp(b.1)));
    hits.into_iter().take(limit).map(|(_, n)| n).collect()
}

/// What a pick writes: the bare name when no other note shares it, else the
/// path (both without `.md`).
pub fn link_name(notes: &[String], pick: &str) -> String {
    let path = pick.trim_end_matches(".md");
    let stem = path.rsplit('/').next().unwrap_or(path);
    let same = notes.iter().filter(|n| n.trim_end_matches(".md").rsplit('/').next().is_some_and(|s| s.eq_ignore_ascii_case(stem))).count();
    if same > 1 {
        path.to_string()
    } else {
        stem.to_string()
    }
}

/// The `[[` completion: the notes of the vault (scanned on a worker, never
/// on the typing path) and the popup state.
#[derive(Default)]
pub struct Completion {
    root: Option<PathBuf>,
    notes: Vec<String>,
    files: Vec<String>,
    scanned: Option<std::time::Instant>,
    rx: Option<std::sync::mpsc::Receiver<(PathBuf, Vec<String>)>>,
    view: ito_palette::InlineView,
    /// The `[[` the user closed with Esc: it stays closed until a new one.
    dismissed: Option<usize>,
    last_query: String,
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Note,
    Path,
    Image,
}

impl Completion {
    fn refresh(&mut self, ctx: &egui::Context, file: &Path) {
        if let Some(rx) = &self.rx {
            match rx.try_recv() {
                Ok((root, files)) => {
                    if self.root.as_deref() == Some(root.as_path()) {
                        self.notes = files.iter().filter(|f| is_note(f)).cloned().collect();
                        self.files = files;
                    }
                    self.scanned = Some(std::time::Instant::now());
                    self.rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => return,
                Err(_) => self.rx = None,
            }
        }
        let root = root_of(file);
        let stale = self.scanned.is_none_or(|t| t.elapsed().as_secs() >= 5);
        if self.root.as_deref() != Some(root.as_path()) {
            self.notes.clear();
            self.files.clear();
        } else if !stale {
            return;
        }
        self.root = Some(root.clone());
        let (tx, rx) = std::sync::mpsc::channel();
        self.rx = Some(rx);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let files = scan_files(&root);
            let _ = tx.send((root, files));
            ctx.request_repaint();
        });
    }

    /// Show the popup for the caret of `ed` (call before the keymap reads the
    /// keys). Nothing while the input method composes or text is selected.
    pub fn show(&mut self, ctx: &egui::Context, ed: &mut crate::engine::Editor, caret: egui::Rect, composing: bool) {
        let sel = ed.selection();
        let Some(file) = ed.path().map(Path::to_path_buf) else { return };
        let idle = composing || sel.anchor != sel.head;
        let text = ed.text();
        let mut trig = if idle { None } else { trigger(text, ed.caret()).map(|(i, q)| (i, q, Mode::Note)) };
        if trig.is_none() && !idle {
            trig = path_trigger(text, ed.caret()).map(|(i, q, img)| (i, q, if img { Mode::Image } else { Mode::Path }));
        }
        let Some((start, query, mode)) = trig else {
            self.dismissed = None;
            return;
        };
        if self.dismissed == Some(start) {
            return;
        }
        self.refresh(ctx, &file);
        let query = query.to_string();
        if query != self.last_query {
            self.view.selected = 0;
            self.last_query = query.clone();
        }
        let names: Vec<String> = match mode {
            Mode::Note if query.is_empty() => self.notes.iter().take(8).cloned().collect(),
            Mode::Note => suggest(&self.notes, &query, 8).into_iter().map(str::to_string).collect(),
            Mode::Path | Mode::Image => {
                let root = self.root.clone().unwrap_or_default();
                let from_dir = file
                    .parent()
                    .and_then(|d| d.strip_prefix(&root).ok())
                    .map(|d| d.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_default();
                let pool: Vec<String> =
                    self.files.iter().filter(|f| mode == Mode::Path || is_image(f)).map(|f| relative(&from_dir, f)).collect();
                if query.is_empty() {
                    pool.into_iter().take(8).collect()
                } else {
                    suggest_paths(&pool, &query, 8).into_iter().map(str::to_string).collect()
                }
            }
        };
        let entries: Vec<ito_palette::Entry<String>> = names
            .iter()
            .map(|n| ito_palette::Entry::new(if mode == Mode::Note { n.trim_end_matches(".md") } else { n.as_str() }, "", n.clone()))
            .collect();
        let c = ito_theme::colors();
        let style = ito_palette::Style {
            panel: c.raised(),
            border: c.border,
            text: c.fg,
            dim: c.dim,
            selected_bg: c.chosen(),
            selected_text: c.strong(),
            hit: c.wait,
            accent: c.done,
        };
        match ito_palette::show_at(ctx, caret, &mut self.view, &entries, &style) {
            Some(ito_palette::Answer::Pick(pick)) => {
                let (name, tail) = match mode {
                    Mode::Note => (link_name(&self.notes, &pick), "]]"),
                    _ => (pick.replace(' ', "%20"), ")"),
                };
                let text = ed.text();
                let end = ed.caret() + if text[ed.caret()..].starts_with(tail) { tail.len() } else { 0 };
                let at = start + name.len() + tail.len();
                ed.replace(&[(start..end, format!("{name}{tail}"))], ito_textbuf::Selection { anchor: at, head: at });
                self.dismissed = None;
            }
            Some(ito_palette::Answer::Close) => self.dismissed = Some(start),
            _ => {}
        }
    }
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

    #[test]
    fn trigger_needs_an_open_link_on_the_caret_line() {
        assert_eq!(trigger("see [[no", 8), Some((6, "no")));
        assert_eq!(trigger("see [[no", 6), Some((6, "")));
        assert_eq!(trigger("[[a]] b", 7), None);
        assert_eq!(trigger("[[a\nb", 5), None);
        assert_eq!(trigger("[[a#h", 5), None);
    }

    #[test]
    fn link_at_finds_the_link_under_the_caret() {
        let t = "a [[b#H|x]] c [[d]]";
        assert_eq!(link_at(t, 5).map(|l| l.target), Some("b".to_string()));
        assert_eq!(link_at(t, 11).map(|l| l.target), Some("b".to_string()), "just after the brackets still counts");
        assert!(link_at(t, 12).is_none());
        assert_eq!(link_at(t, 16).and_then(|l| l.heading), None);
    }

    #[test]
    fn path_trigger_knows_images_and_relative_paths() {
        assert_eq!(path_trigger("a ![x](im", 9), Some((7, "im", true)));
        assert_eq!(path_trigger("[x](", 4), Some((4, "", false)));
        assert_eq!(path_trigger("[x](a b", 7), None);
        assert_eq!(path_trigger("[x](a) z", 8), None);
        assert_eq!(relative("", "img/a.png"), "img/a.png");
        assert_eq!(relative("notes", "img/a.png"), "../img/a.png");
        assert_eq!(relative("notes/x", "notes/b.md"), "../b.md");
        assert_eq!(relative("notes", "notes/img/a.png"), "img/a.png");
    }

    #[test]
    fn link_name_is_the_stem_unless_it_is_shared() {
        let n: Vec<String> = ["a/x.md", "b/x.md", "y.md"].iter().map(|s| s.to_string()).collect();
        assert_eq!(link_name(&n, "a/x.md"), "a/x");
        assert_eq!(link_name(&n, "y.md"), "y");
    }
}
