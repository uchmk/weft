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
    /// The version written: the save marks the document clean only if it is
    /// still at this version when the write lands.
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
    locked: Option<Locked>,
    /// The column (in graphemes) up and down aim for, kept over short lines.
    goal: Option<usize>,
    /// The spaces Tab puts in (and Shift+Tab takes away) outside tables and
    /// lists: the settings' tab width.
    tab: usize,
}

/// The tab width a document starts with, until the settings say otherwise.
pub const TAB_WIDTH: usize = 4;

impl Editor {
    /// A document of `text` with no file behind it.
    pub fn new(text: &str) -> Editor {
        Editor { doc: Doc::new(text), path: None, locked: None, goal: None, tab: TAB_WIDTH }
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
    /// Whether the text differs from the file: undo back to what was saved
    /// is clean again.
    pub fn is_dirty(&self) -> bool {
        self.doc.is_dirty()
    }
    /// How many lines the text has.
    pub fn line_count(&self) -> usize {
        self.doc.buffer().line_count()
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
        self.doc.buffer().line_col(self.caret())
    }

    /// The line (from 0) holding `byte`.
    pub fn line_of(&self, byte: usize) -> usize {
        self.doc.buffer().line_of(byte)
    }
    /// The bytes of line `line` without its break; `None` past the last line.
    pub fn line_range(&self, line: usize) -> Option<std::ops::Range<usize>> {
        self.doc.buffer().line_range(line)
    }

    /// The caret as `$EDITOR` takes it: the line and the column from 1, the
    /// column in characters and in bytes (ito-openin picks what the editor's
    /// `[line_args]` asks for).
    pub fn editor_pos(&self) -> ito_openin::Pos {
        let text = self.text();
        let at = self.caret();
        let line = self.line_of(at);
        let start = self.line_range(line).map_or(0, |r| r.start);
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

    /// Put the caret at `line`, `col`, both from 1, the column in characters
    /// (`+LINE:COL` as kura's opener passes it, counted as ito-openin's
    /// `Col::in_line` counts). A line past the end is the last line, a column
    /// past the line's end is its end (before the break), and a column inside
    /// a grapheme cluster (`👍🏽`) goes back to the cluster's start.
    pub fn go_to(&mut self, line: usize, col: usize) {
        let text = self.text();
        let line = line.saturating_sub(1).min(self.line_count() - 1);
        let r = self.line_range(line).unwrap_or(text.len()..text.len());
        let b = r.start + pos::byte_of_char(&text[r], col.saturating_sub(1));
        let b = pos::next_grapheme(text, b).and_then(|n| pos::prev_grapheme(text, n)).unwrap_or(b);
        self.set_caret(b, false);
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
            Motion::LineStart => self.line_range(self.line_of(head)).map_or(0, |r| r.start),
            Motion::LineEnd => self.line_range(self.line_of(head)).map_or(text.len(), |r| r.end),
            Motion::DocStart => 0,
            Motion::DocEnd => text.len(),
            Motion::Up => self.vertical(-1),
            Motion::Down => self.vertical(1),
            Motion::PageUp(n) => self.vertical(-(n.max(1) as isize)),
            Motion::PageDown(n) => self.vertical(n.max(1) as isize),
        };
        if vertical {
            if self.goal.is_none() {
                self.goal = Some(self.doc.buffer().line_col(head).1);
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
        let buf = self.doc.buffer();
        let (line, col) = buf.line_col(self.caret());
        let goal = self.goal.unwrap_or(col);
        let target = line as isize + lines;
        if target < 0 {
            return 0;
        }
        let target = target as usize;
        if target >= buf.line_count() {
            return buf.len();
        }
        buf.byte_at(target, goal)
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
    /// Backspaces one after another (within a second, each where the last
    /// stopped) are one undo step.
    pub fn backspace(&mut self, now_ms: u64) -> bool {
        let sel = self.doc.selection();
        let range = if sel.is_empty() {
            match pos::prev_grapheme(self.text(), sel.head) {
                Some(p) => p..sel.head,
                None => return false,
            }
        } else {
            sel.range()
        };
        self.delete_range(range, now_ms)
    }

    /// Delete the selection, or the grapheme after the caret. Deletes one
    /// after another are one undo step, as Backspaces are.
    pub fn delete(&mut self, now_ms: u64) -> bool {
        let sel = self.doc.selection();
        let range = if sel.is_empty() {
            match pos::next_grapheme(self.text(), sel.head) {
                Some(n) => sel.head..n,
                None => return false,
            }
        } else {
            sel.range()
        };
        self.delete_range(range, now_ms)
    }

    /// A selection deleted is a step of its own; a grapheme joins the run of
    /// Backspaces or Deletes before it.
    fn delete_range(&mut self, range: std::ops::Range<usize>, now_ms: u64) -> bool {
        if !self.writable() || range.is_empty() {
            return false;
        }
        self.goal = None;
        self.doc.deleted(range, now_ms).is_ok()
    }

    /// The selected text, for Copy.
    pub fn selected_text(&self) -> Option<String> {
        let sel = self.doc.selection();
        (!sel.is_empty()).then(|| self.text()[sel.range()].to_owned())
    }

    /// Take the selection out, for Cut: an undo step of its own.
    pub fn cut(&mut self, now_ms: u64) -> Option<String> {
        if !self.writable() {
            return None;
        }
        let taken = self.selected_text()?;
        self.delete_range(self.doc.selection().range(), now_ms).then_some(taken)
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

    /// The spaces [`Editor::indent`] and [`Editor::outdent`] take when the
    /// command does not say.
    pub fn tab_width(&self) -> usize {
        self.tab
    }
    pub fn set_tab_width(&mut self, n: usize) {
        self.tab = n.max(1);
    }

    /// The lines a Tab or Shift+Tab works on when the selection crosses a
    /// line break: every line it touches, less the last when it ends at that
    /// line's start. `None` when the selection is within one line.
    fn selected_lines(&self) -> Option<std::ops::RangeInclusive<usize>> {
        let r = self.doc.selection().range();
        let (first, last) = (self.line_of(r.start), self.line_of(r.end));
        if first == last {
            return None;
        }
        let ends_at_start = self.line_range(last).is_some_and(|l| l.start == r.end);
        Some(first..=if ends_at_start { last - 1 } else { last })
    }

    /// Tab outside a table and a list, as one undo step: `width` spaces over
    /// the selection, or, when the selection crosses a line break, `width`
    /// spaces before each of its lines (empty lines are left alone).
    pub fn indent(&mut self, width: usize) -> bool {
        if !self.writable() || width == 0 {
            return false;
        }
        let before = self.doc.selection();
        let pad = " ".repeat(width);
        let tx = match self.selected_lines() {
            None => {
                let r = before.range();
                Transaction::new(vec![Edit { range: r.clone(), text: pad }], before, Selection::caret(r.start + width))
            }
            Some(lines) => {
                let starts: Vec<usize> = lines.filter_map(|l| self.line_range(l)).filter(|r| !r.is_empty()).map(|r| r.start).collect();
                if starts.is_empty() {
                    return false;
                }
                // A position at a line's start stays there, so the selection
                // takes in the spaces put before its first line.
                let shift = |p: usize| p + width * starts.iter().filter(|&&s| s < p).count();
                let after = Selection { anchor: shift(before.anchor), head: shift(before.head) };
                // From the last line up: each edit leaves the lines above where they were.
                let edits = starts.iter().rev().map(|&s| Edit { range: s..s, text: pad.clone() }).collect();
                Transaction::new(edits, before, after)
            }
        };
        self.goal = None;
        self.doc.apply(tx).is_ok()
    }

    /// Shift+Tab outside a table and a list, as one undo step: up to `width`
    /// spaces off the start of the caret's line, or of each line of a
    /// selection that crosses a line break. `false` when no line starts with
    /// a space.
    pub fn outdent(&mut self, width: usize) -> bool {
        if !self.writable() || width == 0 {
            return false;
        }
        let before = self.doc.selection();
        let lines = self.selected_lines().unwrap_or_else(|| {
            let l = self.line_of(before.head);
            l..=l
        });
        let text = self.text();
        let cuts: Vec<(usize, usize)> = lines
            .filter_map(|l| self.line_range(l))
            .map(|r| (r.start, text[r].bytes().take_while(|&b| b == b' ').take(width).count()))
            .filter(|&(_, n)| n > 0)
            .collect();
        if cuts.is_empty() {
            return false;
        }
        let shift = |p: usize| p - cuts.iter().map(|&(s, n)| p.saturating_sub(s).min(n)).sum::<usize>();
        let after = Selection { anchor: shift(before.anchor), head: shift(before.head) };
        let edits = cuts.iter().rev().map(|&(s, n)| Edit { range: s..s + n, text: String::new() }).collect();
        self.goal = None;
        self.doc.apply(Transaction::new(edits, before, after)).is_ok()
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

    /// A save of `version` reached the disk. It marks the text clean only when
    /// nothing changed while it was written; otherwise what is on screen is
    /// not what the file holds, and it stays changed until the next save.
    pub fn saved(&mut self, version: u64) {
        if self.doc.version() == version {
            self.doc.mark_saved();
        }
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
        assert!(e.backspace(0));
        assert_eq!(e.text(), "x\r\ny");
        assert!(e.delete(10));
        assert_eq!(e.text(), "xy", "the CRLF goes as one");
        assert!(e.undo());
        assert_eq!(e.text(), "x\r\ny", "a Delete does not join the Backspace before it");
        assert!(e.undo());
        assert_eq!(e.text(), "x👨‍👩‍👧\r\ny");
        assert!(!at("", 0).backspace(0));
    }

    #[test]
    fn backspaces_in_a_row_are_one_step_until_a_pause() {
        let mut e = at("日本語abc", "日本語abc".len());
        for t in [0, 100, 200] {
            assert!(e.backspace(t));
        }
        assert_eq!(e.text(), "日本語");
        assert!(e.backspace(5000), "after a pause, a new step");
        assert_eq!(e.text(), "日本");
        assert!(e.undo());
        assert_eq!(e.text(), "日本語");
        assert!(e.undo());
        assert_eq!(e.text(), "日本語abc", "the three Backspaces come back as one");
        let mut e = at("abcd", 0);
        assert!(e.delete(0));
        assert!(e.delete(100));
        e.seal();
        assert!(e.delete(200), "a seal (a command) ends the run");
        assert_eq!(e.text(), "d");
        assert!(e.undo());
        assert!(e.undo());
        assert_eq!(e.text(), "abcd");
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
        assert_eq!(e.cut(0).as_deref(), Some("日"));
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
    fn go_to_takes_the_line_and_a_char_column_from_one_and_clamps_them() {
        let mut e = Editor::new("# t\r\n漢字👍🏽x\r\nend");
        e.go_to(2, 3);
        assert_eq!(&e.text()[e.caret()..], "👍🏽x\r\nend", "漢字 are two chars, not six bytes");
        e.go_to(2, 4);
        assert_eq!(&e.text()[e.caret()..], "👍🏽x\r\nend", "inside 👍🏽 (two chars) goes to its start");
        e.go_to(2, 5);
        assert_eq!(&e.text()[e.caret()..], "x\r\nend");
        assert_eq!(e.line_col(), (1, 3), "the status counts graphemes: 👍🏽 is one");
        let p = e.editor_pos();
        e.go_to(1, 1);
        assert_eq!(e.caret(), 0);
        e.go_to(p.line, p.col.expect("a column").chars);
        assert_eq!(&e.text()[e.caret()..], "x\r\nend", "the caret $EDITOR gets comes back");
        e.go_to(2, 99);
        assert_eq!(e.caret(), "# t\r\n漢字👍🏽x".len(), "past the line's end is its end, before the CRLF");
        e.go_to(1, 99);
        assert_eq!(e.caret(), 3, "never inside the CRLF");
        e.go_to(99, 2);
        assert_eq!(&e.text()[e.caret()..], "nd", "past the last line is the last line");
        let mut e = Editor::new("");
        e.go_to(5, 5);
        assert_eq!(e.caret(), 0, "an empty file has its one line");
    }

    #[test]
    fn dirty_follows_undo_to_the_save_and_a_save_job_keeps_bom_and_crlf() {
        let mut e = Editor::new("\u{feff}a\r\nb");
        assert!(!e.is_dirty());
        assert_eq!(e.save_job().err(), Some(NoSave::NoPath));
        e.path = Some(PathBuf::from("x.md"));
        e.set_caret(e.text().len(), false);
        e.insert("x", 0);
        assert!(e.is_dirty());
        assert!(e.undo());
        assert!(!e.is_dirty(), "undo back to the text as read is clean");
        assert!(e.redo());
        e.newline(0);
        let job = e.save_job().expect("a job");
        assert_eq!(job.bytes, "\u{feff}a\r\nbx\r\n");
        e.saved(job.version);
        assert!(!e.is_dirty());
        e.insert("y", 10);
        assert!(e.is_dirty());
        assert!(e.undo());
        assert!(!e.is_dirty(), "undo back to the saved text is clean");
        assert!(e.undo());
        assert!(e.is_dirty(), "undo past the save is not");
        assert!(e.redo());
        assert!(!e.is_dirty());
    }

    #[test]
    fn a_save_that_lands_after_an_edit_leaves_the_text_changed() {
        let mut e = Editor::new("a");
        e.path = Some(PathBuf::from("x.md"));
        e.set_caret(1, false);
        e.insert("b", 0);
        let job = e.save_job().expect("a job");
        e.insert("c", 5000);
        e.saved(job.version);
        assert!(e.is_dirty(), "the file has ab, the window abc");
        let job = e.save_job().expect("a job");
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
