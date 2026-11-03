//! The Document Engine as weft drives it: ito-textbuf's `Doc` (the text, the
//! selection and the undo history) with the file it came from, the caret's
//! goal column, and the moves and edits the commands make. No UI and no clock
//! (the caller passes the time): the tests here are the engine's.
//!
//! Positions are bytes on `char` boundaries, as ito-textbuf keeps them; every
//! conversion goes through `ito_textbuf::pos`.

use std::path::{Path, PathBuf};

use std::ops::{Range, RangeInclusive};

use ito_mdedit::{Change, Dir, Step, Target};
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
    /// Tab puts in a tab character instead of `tab` spaces (the settings).
    tab_char: bool,
}

/// The tab width a document starts with, until the settings say otherwise.
pub const TAB_WIDTH: usize = 4;

impl Editor {
    /// A document of `text` with no file behind it.
    pub fn new(text: &str) -> Editor {
        Editor { doc: Doc::new(text), path: None, locked: None, goal: None, tab: TAB_WIDTH, tab_char: false }
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

    /// The spaces Tab puts in outside a table and a list item, and the most
    /// Shift+Tab takes off a line. (Only the tests ask: Tab reads `self.tab`.)
    #[cfg(test)]
    pub fn tab_width(&self) -> usize {
        self.tab
    }
    pub fn set_tab_width(&mut self, n: usize) {
        self.tab = n.max(1);
    }
    /// Whether Tab puts in a tab character instead of the spaces.
    pub fn set_tab_char(&mut self, on: bool) {
        self.tab_char = on;
    }

    /// What Tab puts in outside a table and a list item.
    fn unit(&self) -> String {
        if self.tab_char {
            '\t'.to_string()
        } else {
            " ".repeat(self.tab)
        }
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

    /// Whether the selection crosses a line break, so that Tab and Shift+Tab
    /// work on its lines ([`Editor::shift_lines`]) and not at the caret.
    pub fn spans_lines(&self) -> bool {
        self.selected_lines().is_some()
    }

    /// Tab outside a table and a list item, as one undo step: a tab
    /// character or the tab width in spaces over the selection.
    pub fn indent(&mut self) -> bool {
        if !self.writable() {
            return false;
        }
        let before = self.doc.selection();
        let (r, unit) = (before.range(), self.unit());
        let after = Selection::caret(r.start + unit.len());
        self.goal = None;
        self.doc.apply(Transaction::new(vec![Edit { range: r, text: unit }], before, after)).is_ok()
    }

    /// Shift+Tab outside a table and a list item, as one undo step: a tab,
    /// or up to the tab width of spaces, off the start of the caret's line.
    /// `false` when the line starts with neither.
    pub fn outdent(&mut self) -> bool {
        if !self.writable() {
            return false;
        }
        let before = self.doc.selection();
        let Some(line) = self.line_range(self.line_of(before.head)) else { return false };
        let (s, n) = (line.start, unindent_len(&self.text()[line], self.tab));
        if n == 0 {
            return false;
        }
        // A position among the blanks taken away goes to the line's start.
        let shift = |p: usize| p - p.saturating_sub(s).min(n);
        let after = Selection { anchor: shift(before.anchor), head: shift(before.head) };
        self.goal = None;
        self.doc.apply(Transaction::new(vec![Edit { range: s..s + n, text: String::new() }], before, after)).is_ok()
    }

    /// Tab (`deeper`) or Shift+Tab over a selection that crosses a line
    /// break, as one undo step. Each line it touches moves one level, but a
    /// blank one: a list item as [`ito_mdedit::indent_item`] and
    /// [`ito_mdedit::outdent_item`] move it, with its nested lines (so an
    /// item under one selected goes with it, not twice); any other line by a
    /// tab or the tab width of spaces at its start, a line of code too. The
    /// rows of a table stay (spaces before a table at the left edge make it
    /// code) unless they are in a selected item. The selection keeps its
    /// lines, so the next press goes on. `false` when nothing moved.
    pub fn shift_lines(&mut self, deeper: bool) -> bool {
        if !self.writable() {
            return false;
        }
        let Some(lines) = self.selected_lines() else { return false };
        let before = self.doc.selection();
        let old = self.text();
        let new = shift_text(old, lines, deeper, &self.unit(), self.tab);
        let (os, ns) = (line_starts(old), line_starts(&new));
        // The structure commands keep the lines; if they did not, the
        // selection could not be kept on its lines.
        if new == old || os.len() != ns.len() {
            return false;
        }
        let span = |t: &str, st: &[usize], i: usize| st[i]..st.get(i + 1).copied().unwrap_or(t.len());
        // Line by line (with its break): the old range, the new, and the
        // bytes the two share at the start and at the end.
        let pairs: Vec<_> = (0..os.len())
            .map(|i| {
                let (o, n) = (span(old, &os, i), span(&new, &ns, i));
                let same = common(&old[o.clone()], &new[n.clone()]);
                (o, n, same)
            })
            .collect();
        // From the last line up: each edit leaves the lines above where they were.
        let edits: Vec<Edit> = pairs
            .iter()
            .rev()
            .filter(|(o, n, (p, s))| o.len() > p + s || n.len() > p + s)
            .map(|(o, n, (p, s))| Edit { range: o.start + p..o.end - s, text: new[n.start + p..n.end - s].to_owned() })
            .collect();
        // A position keeps its place in what did not change; one at a line's
        // start stays there, so the selection takes in what went before the
        // first line; one in what changed goes to the end of what took its place.
        let map = |x: usize| {
            let (o, n, (p, s)) = &pairs[os.partition_point(|&st| st <= x) - 1];
            let c = x - o.start;
            n.start
                + if c <= *p {
                    c
                } else if c >= o.len() - s {
                    n.len() - (o.len() - c)
                } else {
                    c.min(n.len() - s)
                }
        };
        let after = Selection { anchor: map(before.anchor), head: map(before.head) };
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

// ---------------------------------------------------------------- Tab over lines

/// `text` after Tab (`deeper`) or Shift+Tab on `lines`, as
/// [`Editor::shift_lines`] says. A line that is not a list item gets `unit`
/// at its start, or loses a tab or up to `tab` spaces.
fn shift_text(text: &str, lines: RangeInclusive<usize>, deeper: bool, unit: &str, tab: usize) -> String {
    let mut cur = text.to_owned();
    // The other lines move last, so the items and tables are found in a
    // text where only the items above have moved (the lines of a fence put
    // four spaces in would no longer be a fence).
    let mut plain = Vec::new();
    // Lines up to here went with an item, or are the rows of a table.
    let mut taken_to: Option<usize> = None;
    // Lines up to here look like a table but are not one (they are code).
    let mut not_table_to: Option<usize> = None;
    for l in lines {
        if taken_to.is_some_and(|t| l <= t) {
            continue;
        }
        let ls = line_ranges(&cur);
        let Some(r) = ls.get(l).cloned() else { break };
        let line = &cur[r.clone()];
        if line.trim().is_empty() {
            continue;
        }
        if starts_item(line) {
            let moved = if deeper { ito_mdedit::indent_item(&cur, r.start) } else { ito_mdedit::outdent_item(&cur, r.start) };
            // `None` for a line of code, and for an item at the left edge
            // going out: that one stays, and so do its nested lines.
            if moved.is_some() || (!deeper && ito_mdedit::indent_item(&cur, r.start).is_some()) {
                taken_to = Some(extent(&cur, &ls, l));
                if let Some(c) = moved {
                    cur = c.apply(&cur);
                }
                continue;
            }
        } else if line.contains('|') && !not_table_to.is_some_and(|n| l <= n) {
            if let Some(c) = ito_mdedit::next_cell(&cur, r.start, Step::Next) {
                let (header, last) = (line_index(&ls, c.range.start), line_index(&ls, c.range.end));
                let h = ls[header].clone();
                // ito-mdedit's tables are lines with bars; a table is one only
                // where the Markdown reads one (not in a code fence).
                if ito_mdedit::next(&cur, h.end, Target::Table, Dir::Up).is_some_and(|s| s >= h.start) {
                    taken_to = Some(last);
                    continue;
                }
                not_table_to = Some(last);
            }
        }
        plain.push(l);
    }
    let ls = line_ranges(&cur);
    for &l in plain.iter().rev() {
        let r = ls[l].clone();
        if deeper {
            cur.insert_str(r.start, unit);
        } else {
            let n = unindent_len(&cur[r.clone()], tab);
            cur.replace_range(r.start..r.start + n, "");
        }
    }
    cur
}

/// What Shift+Tab takes off the start of `line`: a tab, or up to `tab` spaces.
fn unindent_len(line: &str, tab: usize) -> usize {
    if line.starts_with('\t') {
        1
    } else {
        line.bytes().take_while(|&b| b == b' ').take(tab).count()
    }
}

/// Whether `line` starts a list item as ito-mdedit reads one: blanks, then
/// `-`, `*` or `+` and a space, or 1 to 9 digits, `.` or `)` and a space.
fn starts_item(line: &str) -> bool {
    let b = line.trim_start_matches([' ', '\t']).as_bytes();
    if b.len() >= 2 && matches!(b[0], b'-' | b'*' | b'+') && b[1] == b' ' {
        return true;
    }
    let d = b.iter().take_while(|c| c.is_ascii_digit()).count();
    (1..=9).contains(&d) && b.len() > d + 1 && matches!(b[d], b'.' | b')') && b[d + 1] == b' '
}

/// The width of the blanks `line` starts with (a tab to the next multiple of 4).
fn lead(line: &str) -> usize {
    let mut w = 0;
    for b in line.bytes() {
        match b {
            b' ' => w += 1,
            b'\t' => w = (w / 4 + 1) * 4,
            _ => break,
        }
    }
    w
}

/// The last line of the item on line `k`: the lines after it that are more
/// indented (blank lines among them), as ito-mdedit moves them with it.
fn extent(text: &str, ls: &[Range<usize>], k: usize) -> usize {
    let w = lead(&text[ls[k].clone()]);
    let mut last = k;
    for (j, r) in ls.iter().enumerate().skip(k + 1) {
        let line = &text[r.clone()];
        if line.trim().is_empty() {
            continue;
        }
        if lead(line) <= w {
            break;
        }
        last = j;
    }
    last
}

/// Every line of `text` (one after a last break too), without its break.
fn line_ranges(text: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut start = 0;
    for (i, _) in text.match_indices('\n') {
        let end = if i > start && text.as_bytes()[i - 1] == b'\r' { i - 1 } else { i };
        out.push(start..end);
        start = i + 1;
    }
    out.push(start..text.len());
    out
}

/// The line of `ls` holding `byte`.
fn line_index(ls: &[Range<usize>], byte: usize) -> usize {
    ls.partition_point(|r| r.start <= byte).saturating_sub(1)
}

/// Where each line of `text` starts.
fn line_starts(text: &str) -> Vec<usize> {
    std::iter::once(0).chain(text.match_indices('\n').map(|(i, _)| i + 1)).collect()
}

/// The bytes `a` and `b` share at the start and then at the end of what is
/// left, both on `char` boundaries.
fn common(a: &str, b: &str) -> (usize, usize) {
    let mut p = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    while !a.is_char_boundary(p) {
        p -= 1;
    }
    let most = a.len().min(b.len()) - p;
    let mut s = a.bytes().rev().zip(b.bytes().rev()).take(most).take_while(|(x, y)| x == y).count();
    while !a.is_char_boundary(a.len() - s) {
        s -= 1;
    }
    (p, s)
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
