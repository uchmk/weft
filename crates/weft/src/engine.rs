//! The Document Engine as weft drives it: ito-textbuf's `Doc` (the text, the
//! selection and the undo history) with the file it came from, the caret's
//! goal column, and the moves and edits the commands make. No UI and no clock
//! (the caller passes the time): the tests here are the engine's.
//!
//! Positions are bytes on `char` boundaries, as ito-textbuf keeps them; every
//! conversion goes through `ito_textbuf::pos`.

use std::path::{Path, PathBuf};

use ito_mdedit::Change;
use ito_textbuf::{pos, Doc, Edit, Selection, Transaction};

/// A caret move. `PageUp` / `PageDown` carry how many lines a page is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Left,
    Right,
    Up,
    Down,
    LineStart,
    LineEnd,
    DocStart,
    DocEnd,
    PageUp(usize),
    PageDown(usize),
}

/// Why a document cannot be edited.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locked {
    /// The file is not UTF-8: what is shown is a lossy reading, and saving it
    /// would destroy the bytes that did not decode.
    NotUtf8,
}

/// What a save writes, taken on the UI thread and written on another.
pub struct SaveJob {
    pub path: PathBuf,
    /// The BOM it was read with, then the text (its line endings as read).
    pub bytes: String,
    /// The version written: the document is clean while it stays there.
    pub version: u64,
}

/// Why there is nothing to save.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoSave {
    /// A new document that has no file yet.
    NoPath,
    Locked(Locked),
}

pub struct Editor {
    doc: Doc,
    path: Option<PathBuf>,
    /// The version last written or read. Until ito-textbuf keeps a saved mark
    /// (one that undo and redo move back to), undoing back to the saved text
    /// still counts as changed.
    saved_version: u64,
    locked: Option<Locked>,
    /// The column (in graphemes) up and down aim for, kept over short lines.
    goal: Option<usize>,
}

impl Editor {
    /// A document of `text` with no file behind it.
    pub fn new(text: &str) -> Editor {
        let doc = Doc::new(text);
        let saved_version = doc.version();
        Editor { doc, path: None, saved_version, locked: None, goal: None }
    }

    /// Read `path`. A file that is not there yet opens empty (saving creates
    /// it); one that is not UTF-8 opens read-only with what could be decoded.
    pub fn open(path: &Path) -> Result<Editor, String> {
        let (text, locked) = match std::fs::read(path) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(text) => (text, None),
                Err(e) => (String::from_utf8_lossy(e.as_bytes()).into_owned(), Some(Locked::NotUtf8)),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (String::new(), None),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        let mut ed = Editor::new(&text);
        ed.path = Some(path.to_path_buf());
        ed.locked = locked;
        Ok(ed)
    }

    pub fn text(&self) -> &str {
        self.doc.text()
    }
    pub fn version(&self) -> u64 {
        self.doc.version()
    }
    pub fn selection(&self) -> Selection {
        self.doc.selection()
    }
    pub fn caret(&self) -> usize {
        self.doc.selection().head
    }
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
    pub fn locked(&self) -> Option<Locked> {
        self.locked
    }
    pub fn is_dirty(&self) -> bool {
        self.doc.version() != self.saved_version
    }
    /// The line endings and BOM it was read with, for the status bar.
    pub fn eol(&self) -> ito_textbuf::Eol {
        self.doc.buffer().eol()
    }
    pub fn bom(&self) -> bool {
        self.doc.buffer().bom()
    }

    /// The caret's line and column (graphemes), both from 0.
    pub fn line_col(&self) -> (usize, usize) {
        pos::line_col(self.text(), self.caret())
    }

    /// The caret as `$EDITOR` takes it: the line and the column from 1, the
    /// column in characters and in bytes (ito-openin picks what the editor's
    /// `[line_args]` asks for).
    pub fn editor_pos(&self) -> ito_openin::Pos {
        let text = self.text();
        let at = self.caret();
        let line = pos::line_of(text, at);
        let start = text[..at].rfind('\n').map_or(0, |i| i + 1);
        let chars = pos::char_of_byte(&text[start..], at - start);
        ito_openin::Pos::new(line + 1, ito_openin::Col::new(chars + 1, at - start + 1))
    }

    // ------------------------------------------------------------ moving

    /// Put the caret at `byte` (a click), keeping the anchor when `extend`
    /// (a drag, a Shift+click).
    pub fn set_caret(&mut self, byte: usize, extend: bool) {
        let sel = self.doc.selection();
        let anchor = if extend { sel.anchor } else { byte };
        self.doc.set_selection(Selection { anchor, head: byte });
        self.goal = None;
    }

    pub fn select_all(&mut self) {
        self.doc.set_selection(Selection { anchor: 0, head: self.text().len() });
        self.goal = None;
    }

    /// Move the caret; with `extend` the anchor stays (Shift).
    pub fn move_caret(&mut self, m: Motion, extend: bool) {
        let sel = self.doc.selection();
        let text = self.text();
        let head = sel.head;
        let vertical = matches!(m, Motion::Up | Motion::Down | Motion::PageUp(_) | Motion::PageDown(_));
        let to = match m {
            // Without Shift, a selection collapses to the side the arrow points to.
            Motion::Left if !extend && !sel.is_empty() => sel.range().start,
            Motion::Right if !extend && !sel.is_empty() => sel.range().end,
            Motion::Left => pos::prev_grapheme(text, head).unwrap_or(0),
            Motion::Right => pos::next_grapheme(text, head).unwrap_or(text.len()),
            Motion::LineStart => text[..head].rfind('\n').map_or(0, |i| i + 1),
            Motion::LineEnd => {
                let line = pos::line_of(text, head);
                pos::line_range(text, line).map_or(text.len(), |r| r.end)
            }
            Motion::DocStart => 0,
            Motion::DocEnd => text.len(),
            Motion::Up => self.vertical(-1),
            Motion::Down => self.vertical(1),
            Motion::PageUp(n) => self.vertical(-(n.max(1) as isize)),
            Motion::PageDown(n) => self.vertical(n.max(1) as isize),
        };
        if vertical {
            if self.goal.is_none() {
                self.goal = Some(pos::line_col(self.text(), head).1);
            }
        } else {
            self.goal = None;
        }
        let anchor = if extend { sel.anchor } else { to };
        self.doc.set_selection(Selection { anchor, head: to });
    }

    /// Where `lines` lines down (up when negative) lands, at the goal column.
    /// Past the first line is the start of the text, past the last its end.
    fn vertical(&self, lines: isize) -> usize {
        let text = self.text();
        let (line, col) = pos::line_col(text, self.caret());
        let goal = self.goal.unwrap_or(col);
        let target = line as isize + lines;
        if target < 0 {
            return 0;
        }
        let target = target as usize;
        if target >= pos::line_count(text) {
            return text.len();
        }
        pos::byte_at(text, target, goal)
    }

    // ------------------------------------------------------------ editing

    fn writable(&self) -> bool {
        self.locked.is_none()
    }

    /// Type `text` over the selection. Keys typed one after another (within
    /// a second, at the place the last ended) are one undo step.
    pub fn insert(&mut self, text: &str, now_ms: u64) -> bool {
        if !self.writable() || text.is_empty() {
            return false;
        }
        self.goal = None;
        self.doc.typed(text, now_ms).is_ok()
    }

    /// Text an input method committed: an undo step of its own.
    pub fn commit(&mut self, text: &str, now_ms: u64) -> bool {
        if !self.writable() || text.is_empty() {
            return false;
        }
        self.goal = None;
        self.doc.commit(text, now_ms).is_ok()
    }

    /// Break the line with the break the text already uses (CRLF stays CRLF).
    /// The break is an undo step of its own, not the end of the word before it.
    pub fn newline(&mut self, now_ms: u64) -> bool {
        self.doc.seal();
        let nl = self.doc.buffer().newline();
        self.insert(nl, now_ms)
    }

    /// Paste: the line breaks become the text's own, and the paste is one
    /// undo step.
    pub fn paste(&mut self, text: &str, now_ms: u64) -> bool {
        let unified = text.replace("\r\n", "\n");
        let text = match self.doc.buffer().newline() {
            "\n" => unified,
            nl => unified.replace('\n', nl),
        };
        self.commit(&text, now_ms)
    }

    /// Delete the selection, or the grapheme before the caret (a CRLF is one).
    pub fn backspace(&mut self) -> bool {
        let sel = self.doc.selection();
        let range = if sel.is_empty() {
            match pos::prev_grapheme(self.text(), sel.head) {
                Some(p) => p..sel.head,
                None => return false,
            }
        } else {
            sel.range()
        };
        self.delete_range(range)
    }

    /// Delete the selection, or the grapheme after the caret.
    pub fn delete(&mut self) -> bool {
        let sel = self.doc.selection();
        let range = if sel.is_empty() {
            match pos::next_grapheme(self.text(), sel.head) {
                Some(n) => sel.head..n,
                None => return false,
            }
        } else {
            sel.range()
        };
        self.delete_range(range)
    }

    fn delete_range(&mut self, range: std::ops::Range<usize>) -> bool {
        if !self.writable() || range.is_empty() {
            return false;
        }
        self.goal = None;
        let before = self.doc.selection();
        let at = range.start;
        let tx = Transaction::new(vec![Edit { range, text: String::new() }], before, Selection::caret(at));
        self.doc.apply(tx).is_ok()
    }

    /// The selected text, for Copy.
    pub fn selected_text(&self) -> Option<String> {
        let sel = self.doc.selection();
        (!sel.is_empty()).then(|| self.text()[sel.range()].to_owned())
    }

    /// Take the selection out, for Cut.
    pub fn cut(&mut self) -> Option<String> {
        if !self.writable() {
            return None;
        }
        let taken = self.selected_text()?;
        self.delete_range(self.doc.selection().range()).then_some(taken)
    }

    /// Run a structure command of ito-mdedit at the caret, as one undo step.
    /// `false` when it does not apply here (no table under the caret, say).
    pub fn structure(&mut self, command: impl FnOnce(&str, usize) -> Option<Change>) -> bool {
        if !self.writable() {
            return false;
        }
        let before = self.doc.selection();
        let Some(change) = command(self.text(), before.head) else { return false };
        self.goal = None;
        self.doc.apply(Transaction::from_change(&change, before)).is_ok()
    }

    pub fn undo(&mut self) -> bool {
        self.goal = None;
        self.doc.undo().unwrap_or(false)
    }

    pub fn redo(&mut self) -> bool {
        self.goal = None;
        self.doc.redo().unwrap_or(false)
    }

    /// End the typing step in progress (focus lost, a command run).
    pub fn seal(&mut self) {
        self.doc.seal();
    }

    // ------------------------------------------------------------ saving

    /// What to write, as of now.
    pub fn save_job(&self) -> Result<SaveJob, NoSave> {
        if let Some(l) = self.locked {
            return Err(NoSave::Locked(l));
        }
        let path = self.path.clone().ok_or(NoSave::NoPath)?;
        Ok(SaveJob { path, bytes: self.doc.buffer().saved(), version: self.doc.version() })
    }

    /// A save of `version` reached the disk.
    pub fn saved(&mut self, version: u64) {
        self.saved_version = version;
    }
}

/// Write a save job: beside the file first, then over it.
pub fn write(job: &SaveJob) -> Result<(), String> {
    ito_common::write_atomic(&job.path, job.bytes.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str, caret: usize) -> Editor {
        let mut e = Editor::new(text);
        e.set_caret(caret, false);
        e
    }

    #[test]
    fn arrows_step_over_graphemes_crlf_full_width_and_emoji() {
        let mut e = at("a\r\nあ👨‍👩‍👧b", 0);
        e.move_caret(Motion::Right, false);
        assert_eq!(e.caret(), 1);
        e.move_caret(Motion::Right, false);
        assert_eq!(e.caret(), 3, "the CRLF is one step");
        e.move_caret(Motion::Right, false);
        assert_eq!(e.caret(), 6, "あ is three bytes");
        e.move_caret(Motion::Right, false);
        assert_eq!(&e.text()[e.caret()..], "b", "the family is one step");
        e.move_caret(Motion::Left, false);
        assert_eq!(e.caret(), 6);
        e.move_caret(Motion::Left, false);
        e.move_caret(Motion::Left, false);
        assert_eq!(e.caret(), 1);
    }

    #[test]
    fn backspace_and_delete_take_a_whole_grapheme() {
        let mut e = at("x👨‍👩‍👧\r\ny", 0);
        let end_of_family = "x👨‍👩‍👧".len();
        e.set_caret(end_of_family, false);
        assert!(e.backspace());
        assert_eq!(e.text(), "x\r\ny");
        assert!(e.delete());
        assert_eq!(e.text(), "xy", "the CRLF goes as one");
        assert!(e.undo());
        assert!(e.undo());
        assert_eq!(e.text(), "x👨‍👩‍👧\r\ny");
        assert!(!at("", 0).backspace());
    }

    #[test]
    fn up_and_down_keep_the_goal_column_over_a_short_line() {
        let mut e = at("日本語です\nab\nあいうえお", 0);
        e.move_caret(Motion::Right, false);
        e.move_caret(Motion::Right, false);
        e.move_caret(Motion::Right, false);
        e.move_caret(Motion::Down, false);
        assert_eq!(e.line_col(), (1, 2), "the short line stops at its end");
        e.move_caret(Motion::Down, false);
        assert_eq!(e.line_col(), (2, 3), "the goal column comes back");
        e.move_caret(Motion::Down, false);
        assert_eq!(e.caret(), e.text().len(), "past the last line is the end");
        e.move_caret(Motion::DocStart, false);
        e.move_caret(Motion::Up, false);
        assert_eq!(e.caret(), 0);
        e.move_caret(Motion::PageDown(10), false);
        assert_eq!(e.caret(), e.text().len());
    }

    #[test]
    fn home_and_end_stop_before_the_crlf() {
        let mut e = at("ab\r\ncd", 1);
        e.move_caret(Motion::LineEnd, false);
        assert_eq!(e.caret(), 2);
        e.move_caret(Motion::Down, false);
        e.move_caret(Motion::LineStart, false);
        assert_eq!(e.caret(), 4);
        e.move_caret(Motion::LineEnd, true);
        assert_eq!(e.selected_text().as_deref(), Some("cd"));
        e.move_caret(Motion::Left, false);
        assert_eq!(e.caret(), 4, "Left collapses to the start");
    }

    #[test]
    fn typing_is_one_step_and_newline_keeps_crlf() {
        let mut e = at("a\r\nb", 1);
        assert!(e.insert("x", 0));
        assert!(e.insert("y", 100));
        assert!(e.newline(200));
        assert_eq!(e.text(), "axy\r\n\r\nb");
        assert!(e.undo());
        assert_eq!(e.text(), "axy\r\nb");
        assert!(e.undo());
        assert_eq!(e.text(), "a\r\nb");
        assert!(e.redo());
        assert_eq!(e.text(), "axy\r\nb");
    }

    #[test]
    fn paste_uses_the_texts_own_line_breaks() {
        let mut e = at("a\r\nb", 4);
        assert!(e.paste("1\n2\r\n3", 0));
        assert_eq!(e.text(), "a\r\nb1\r\n2\r\n3");
        let mut e = at("a\nb", 3);
        assert!(e.paste("1\r\n2", 0));
        assert_eq!(e.text(), "a\nb1\n2");
        assert!(e.undo());
        assert_eq!(e.text(), "a\nb");
    }

    #[test]
    fn cut_copy_and_select_all() {
        let mut e = at("日本語", 0);
        e.move_caret(Motion::Right, true);
        assert_eq!(e.selected_text().as_deref(), Some("日"));
        assert_eq!(e.cut().as_deref(), Some("日"));
        assert_eq!(e.text(), "本語");
        e.select_all();
        assert_eq!(e.selected_text().as_deref(), Some("本語"));
        assert!(e.insert("x", 0));
        assert_eq!(e.text(), "x");
        assert!(e.undo());
        assert_eq!(e.text(), "本語");
    }

    #[test]
    fn a_structure_command_is_one_step_and_moves_the_caret() {
        let mut e = at("a\n\n- b\n", 0);
        assert!(e.structure(|t, b| ito_mdedit::move_block(t, b, ito_mdedit::Dir::Down)));
        assert_eq!(e.text(), "- b\n\na\n");
        assert!(e.text()[e.caret()..].starts_with('a'), "the caret goes with the block");
        assert!(e.undo());
        assert_eq!(e.text(), "a\n\n- b\n");
        assert!(!e.structure(|_, _| None));
    }

    #[test]
    fn the_editor_position_counts_from_one_in_chars_and_bytes() {
        let e = at("x\r\n日本b", "x\r\n日本".len());
        let p = e.editor_pos();
        assert_eq!(p.line, 2);
        let col = p.col.expect("a column");
        assert_eq!(col.chars, 3);
        assert_eq!(col.bytes, 7);
    }

    #[test]
    fn dirty_follows_the_version_and_a_save_job_keeps_bom_and_crlf() {
        let mut e = Editor::new("\u{feff}a\r\nb");
        assert!(!e.is_dirty());
        assert_eq!(e.save_job().err(), Some(NoSave::NoPath));
        e.path = Some(PathBuf::from("x.md"));
        e.set_caret(e.text().len(), false);
        e.newline(0);
        assert!(e.is_dirty());
        let job = e.save_job().expect("a job");
        assert_eq!(job.bytes, "\u{feff}a\r\nb\r\n");
        e.saved(job.version);
        assert!(!e.is_dirty());
    }

    #[test]
    fn a_file_that_is_not_utf8_opens_locked() {
        let dir = std::env::temp_dir().join(format!("weft-engine-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bad = dir.join("sjis.md");
        std::fs::write(&bad, b"\x82\xa0abc").unwrap();
        let mut e = Editor::open(&bad).unwrap();
        assert_eq!(e.locked(), Some(Locked::NotUtf8));
        assert!(e.text().ends_with("abc"));
        assert!(!e.insert("x", 0));
        assert_eq!(e.save_job().err(), Some(NoSave::Locked(Locked::NotUtf8)));
        let missing = Editor::open(&dir.join("new.md")).unwrap();
        assert_eq!(missing.text(), "");
        assert!(missing.save_job().is_ok());
        let good = dir.join("ok.md");
        std::fs::write(&good, "\u{feff}# t\r\n").unwrap();
        let mut e = Editor::open(&good).unwrap();
        e.set_caret(e.text().len(), false);
        e.insert("x", 0);
        let job = e.save_job().unwrap();
        write(&job).unwrap();
        assert_eq!(std::fs::read_to_string(&good).unwrap(), "\u{feff}# t\r\nx");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
