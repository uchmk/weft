//! The document on screen: the lines in view with their colours, the
//! selection, the caret, the input method's composition and the mouse.
//! A long line scrolls sideways, or folds at the pane's width (word wrap).
//!
//! Only the lines in view are laid out, from the colours the structure worker
//! last read. Those may be a version behind the text; a run is cut to the
//! line and to a character boundary, so old colours never split a character.
//!
//! Two things that break a Markdown layout without showing are marked,
//! quietly: a full-width space (a thin box where it is) and the spaces at a
//! line's end that make a hard break (a faint fill with a dot per space).

use std::ops::Range;
use std::sync::Arc;

use egui::text::{CCursor, LayoutJob, TextFormat};
use egui::{pos2, vec2, Align2, Color32, FontId, Galley, Id, Rect, Sense, Stroke, StrokeKind, Ui};
use ito_textbuf::pos;
use ito_theme::mix;

use crate::engine::Editor;
use crate::structure::Tree;

/// The text's size (monospace).
const SIZE: f32 = 14.0;
/// Space between the gutter's numbers and the text, and before the numbers.
const PAD: f32 = 8.0;

/// A line laid out for this frame.
struct Line {
    /// Its number, from 0.
    n: usize,
    /// Its bytes in the text, the break left out.
    start: usize,
    end: usize,
    /// A break follows it (it is not the last line).
    brk: bool,
    galley: Arc<Galley>,
    /// The composition drawn in it: where (bytes from the line's start) and
    /// how many characters long.
    ime: Option<(usize, usize)>,
    /// Its top, in points below the first line's top.
    off: f32,
    /// Its height: one row, or the rows a wrapped line takes.
    h: f32,
}

impl Line {
    /// The galley's character at `byte` of the text, on this line.
    fn ch(&self, text: &str, byte: usize, with_ime: bool) -> usize {
        let line = &text[self.start..self.end];
        let b = byte.clamp(self.start, self.end) - self.start;
        let mut ch = pos::char_of_byte(line, b);
        if let Some((at, len)) = self.ime {
            if with_ime && b >= at {
                ch += len;
            }
        }
        ch
    }

    /// Where `byte` is (from the galley's top left), on this line.
    fn pos(&self, text: &str, byte: usize, with_ime: bool) -> egui::Pos2 {
        self.galley.pos_from_cursor(CCursor::new(self.ch(text, byte, with_ime))).min
    }

    /// The byte (in the text) nearest to (`x`, `y`) from the galley's top
    /// left, on a grapheme's boundary.
    fn byte_at(&self, text: &str, x: f32, y: f32) -> usize {
        let line = &text[self.start..self.end];
        let mut ch = self.galley.cursor_from_pos(vec2(x, y)).index.0;
        if let Some((at, len)) = self.ime {
            let at = pos::char_of_byte(line, at);
            if ch >= at + len {
                ch -= len;
            } else if ch > at {
                ch = at;
            }
        }
        let b = pos::byte_of_char(line, ch).min(line.len());
        let b = pos::next_grapheme(line, b).and_then(|n| pos::prev_grapheme(line, n)).unwrap_or(b);
        self.start + b
    }

    /// The galley's characters `a..b` as one band per row they are on:
    /// (the row's top, the left x, the right x), from the galley's top left.
    fn bands(&self, a: usize, b: usize) -> Vec<(f32, f32, f32)> {
        let mut out = Vec::new();
        let mut off = 0;
        for placed in &self.galley.rows {
            let n = placed.row.char_count_excluding_newline().0;
            let (s, e) = (a.max(off), b.min(off + n));
            if s < e || (a == b && a >= off && a < off + n.max(1)) {
                let x0 = self.galley.pos_from_cursor(CCursor::new(s)).min.x;
                let x1 = if e >= off + n { placed.rect().max.x } else { self.galley.pos_from_cursor(CCursor::new(e)).min.x };
                out.push((placed.pos.y, x0, x1));
            }
            off += n;
        }
        out
    }
}

/// What lays a line out: the text and its colours, and the width a line wraps
/// at (`None`: it does not wrap).
struct Lay<'a> {
    ui: &'a Ui,
    ed: &'a Editor,
    tree: Option<&'a Tree>,
    colors: &'a ito_theme::Colors,
    plain: &'a TextFormat,
    wrap: Option<f32>,
    row_h: f32,
}

impl Lay<'_> {
    /// Line `n`, `off` points below the top; `comp` is the composition
    /// (the caret's byte, what is composed, how it looks) drawn in it.
    fn line(&self, n: usize, off: f32, comp: Option<(usize, &str, &TextFormat)>) -> Option<Line> {
        let text = self.ed.text();
        let r = self.ed.line_range(n)?;
        let raw = &text[r.clone()];
        let (body, brk) = match raw.strip_suffix('\n') {
            Some(b) => (b.strip_suffix('\r').unwrap_or(b), true),
            None => (raw, false),
        };
        let (start, end) = (r.start, r.start + body.len());
        let at = comp.map(|(caret, _, _)| caret.clamp(start, end) - start);
        let mut job =
            layout(body, self.tree.and_then(|t| t.lines.get(n)), self.colors, self.plain, comp.zip(at).map(|((_, s, f), a)| (a, s, f)));
        if let Some(w) = self.wrap {
            job.wrap.max_width = w;
        }
        let galley = self.ui.painter().layout_job(job);
        let ime = comp.zip(at).map(|((_, s, _), a)| (a, s.chars().count()));
        let h = if self.wrap.is_some() { galley.rows.len().max(1) as f32 * self.row_h } else { self.row_h };
        Some(Line { n, start, end, brk, galley, ime, off, h })
    }

    /// The height of line `n`.
    fn height(&self, n: usize) -> f32 {
        if self.wrap.is_none() {
            return self.row_h;
        }
        self.line(n, 0.0, None).map_or(self.row_h, |l| l.h)
    }
}

pub struct View {
    /// The first line shown, and how far (in points) it is scrolled up out of
    /// the view: less than its height.
    top: usize,
    sub: f32,
    /// How far the text is scrolled sideways, in points (0 when it wraps).
    scroll_x: f32,
    /// The row height `sub` was measured in (0 before the first frame).
    row_h: f32,
    /// How many lines a page moves: what fit last frame, less one.
    page: usize,
    /// Bring the caret into view next frame (it moved or the text changed).
    pub follow: bool,
    /// Where the caret was drawn, for the candidate window.
    caret_rect: Rect,
    ime: ito_ime::Ime,
    /// What changed since the file was read or saved, one entry per line (the
    /// gutter's bars). May be a version behind the text.
    changes: crate::changes::Changes,
}

impl Default for View {
    fn default() -> View {
        View {
            top: 0,
            sub: 0.0,
            scroll_x: 0.0,
            row_h: 0.0,
            page: 20,
            follow: true,
            caret_rect: Rect::NOTHING,
            ime: ito_ime::Ime::new(Id::new("weft-editor")),
            changes: crate::changes::Changes::default(),
        }
    }
}

impl View {
    pub fn page(&self) -> usize {
        self.page
    }

    /// Take the newest marks of what changed (a no-op while they are the same).
    pub fn set_changes(&mut self, c: &crate::changes::Changes) {
        if self.changes.key != c.key {
            self.changes = c.clone();
        }
    }

    /// The line at the top of the view (0 before the first frame).
    pub fn top_line(&self) -> usize {
        self.top
    }

    /// Scroll so `line` is at the top, without bringing the caret into view
    /// (the preview was scrolled and this follows it).
    pub fn scroll_to_line(&mut self, line: usize) {
        if self.row_h > 0.0 {
            self.top = line;
            self.sub = 0.0;
        }
    }

    /// How far down the first line is, as if every line were one row.
    #[cfg(test)]
    fn scroll_y(&self) -> f32 {
        self.top as f32 * self.row_h + self.sub
    }

    /// Carry `sub` over into `top` so it is within the first line, and keep the
    /// first line a line of the text. `h` is the height of a line.
    fn settle(&mut self, total: usize, row_h: f32, h: &dyn Fn(usize) -> f32) {
        self.top = self.top.min(total.saturating_sub(1));
        while self.sub >= h(self.top) && self.top + 1 < total {
            self.sub -= h(self.top);
            self.top += 1;
        }
        while self.sub < 0.0 {
            if self.top == 0 {
                self.sub = 0.0;
                break;
            }
            self.top -= 1;
            self.sub += h(self.top);
        }
        if self.top + 1 >= total {
            self.sub = self.sub.min((h(self.top) - row_h).max(0.0));
        }
    }

    /// Take the keys the input method owns out of this frame while it
    /// composes. Call before the keymap reads the keys.
    pub fn swallow(&self, ctx: &egui::Context) {
        self.ime.swallow_keys(ctx);
    }

    /// Whether the input method is composing in the document, as of the
    /// last frame (for the status bar).
    pub fn composing(&self, ctx: &egui::Context) -> bool {
        self.ime.composing(ctx)
    }

    /// Whether the input method is on and its input mode (for the status
    /// bar); `None` where the platform cannot tell.
    pub fn ime_status(&self, ctx: &egui::Context) -> Option<ito_ime::Status> {
        self.ime.status(ctx)
    }

    /// Draw `ed` in what is left of `ui`. With `keys` the document has the
    /// keys: the input method and the mouse go to it. Typing, copy and paste
    /// and the input method's commits come in with the keys, in their order,
    /// before this; here the composition is drawn and the candidate window
    /// placed. With `wrap` a long line is folded at the width of the pane
    /// instead of scrolling sideways.
    pub fn show(&mut self, ui: &mut Ui, ed: &mut Editor, tree: Option<&Tree>, finds: &[std::ops::Range<usize>], keys: bool, wrap: bool) {
        let rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(rect, Sense::click_and_drag());
        let c = ito_theme::colors();
        let font = FontId::monospace(SIZE);
        let (row_h, char_w) = ui.ctx().fonts_mut(|f| (f.row_height(&font), f.glyph_width(&font, '0')));
        // A new face (the fonts come in after the first frame) keeps the same
        // line at the top and the caret in view: a `+LINE` far down stays shown.
        if self.row_h > 0.0 && self.row_h != row_h {
            self.sub *= row_h / self.row_h;
            self.follow = true;
        }
        self.row_h = row_h;

        // --- The composition. Its commits went in with the keys (`f.commits`
        // is not inserted again); a commit held for its echo is not drawn,
        // the document has it.
        let mut preedit = String::new();
        if keys {
            let f = self.ime.frame(ui, self.caret_rect, ed.caret() as u64);
            if f.composing {
                preedit = f.preedit;
                self.follow = true;
            }
        }

        let text = ed.text();
        let total = ed.line_count();
        let digits = total.to_string().len().max(2);
        let gutter = digits as f32 * char_w + 2.0 * PAD;
        // The minimap takes the right edge when the pane is wide enough for both.
        let map = tree.filter(|t| ito_minimap::fits(t.rows.len(), rect.width() - gutter, char_w));
        let strip_w = map.map_or(0.0, |_| ito_minimap::strip_width(char_w));
        let text_rect = Rect::from_min_max(pos2(rect.left() + gutter, rect.top()), pos2(rect.right() - strip_w, rect.bottom()));
        let strip = Rect::from_min_max(pos2(text_rect.right(), rect.top()), rect.max);
        let text_left = text_rect.left() + PAD;
        let view_w = (text_rect.width() - 2.0 * PAD).max(char_w);
        let view_h = text_rect.height();

        let plain = TextFormat::simple(font.clone(), c.fg);
        let ime_format = TextFormat { background: c.selection(), underline: Stroke::new(1.0, c.ime_underline()), ..plain.clone() };
        let caret_line = ed.line_of(ed.caret());
        let caret_byte = ed.caret();
        let comp = |n: usize| (n == caret_line && !preedit.is_empty()).then_some((caret_byte, preedit.as_str(), &ime_format));
        let lay = Lay { ui, ed, tree, colors: &c, plain: &plain, wrap: wrap.then_some(view_w), row_h };
        let h = |n: usize| lay.height(n);

        // --- Scrolling: the wheel, then the caret.
        if keys && response.hovered() {
            let (d, command) = ui.input(|i| (i.smooth_scroll_delta, i.modifiers.command));
            if !command {
                self.sub -= d.y;
                self.scroll_x -= d.x;
            }
        }
        self.settle(total, row_h, &h);
        if self.follow {
            if let Some(l) = lay.line(caret_line, 0.0, comp(caret_line)) {
                let cy = if wrap { l.pos(text, caret_byte, true).y } else { 0.0 };
                if caret_line < self.top || (caret_line == self.top && cy < self.sub) {
                    self.top = caret_line;
                    self.sub = cy;
                } else {
                    // How far below the top of the view the caret's row ends.
                    let rows = (view_h / row_h).ceil() as usize + 1;
                    let bottom = if caret_line >= self.top + rows {
                        f32::INFINITY
                    } else {
                        (self.top..caret_line).map(&h).sum::<f32>() - self.sub + cy + row_h
                    };
                    if bottom > view_h {
                        // The top that puts the caret's row at the bottom.
                        let mut acc = cy + row_h;
                        let mut n = caret_line;
                        loop {
                            if acc >= view_h {
                                self.top = n;
                                self.sub = acc - view_h;
                                break;
                            }
                            if n == 0 {
                                self.top = 0;
                                self.sub = 0.0;
                                break;
                            }
                            n -= 1;
                            acc += h(n);
                        }
                    }
                }
            }
        }
        self.settle(total, row_h, &h);
        if wrap {
            self.scroll_x = 0.0;
        }
        self.scroll_x = self.scroll_x.max(0.0);

        // --- The lines in view.
        let y0 = text_rect.top() - self.sub;
        let mut lines: Vec<Line> = Vec::new();
        let mut off = 0.0;
        for n in self.top..total {
            if off - self.sub >= view_h {
                break;
            }
            let Some(l) = lay.line(n, off, comp(n)) else { break };
            off += l.h;
            lines.push(l);
        }

        // Sideways, now that the caret's line is laid out.
        if self.follow {
            if let (false, Some(l)) = (wrap, lines.iter().find(|l| l.n == caret_line)) {
                let x = l.pos(text, caret_byte, true).x;
                if x < self.scroll_x {
                    self.scroll_x = (x - 4.0 * char_w).max(0.0);
                } else if x + char_w > self.scroll_x + view_w {
                    self.scroll_x = x + 4.0 * char_w - view_w;
                }
            }
            self.follow = false;
        }

        // --- The mouse: a click puts the caret, a drag (or Shift+click) selects.
        if keys && !lines.is_empty() && response.is_pointer_button_down_on() && !ui.rect_contains_pointer(strip) {
            let (pressed, primary, shift) = ui.input(|i| (i.pointer.primary_pressed(), i.pointer.primary_down(), i.modifiers.shift));
            if let (true, Some(p)) = (primary, response.interact_pointer_pos()) {
                let l = lines.iter().find(|l| p.y < y0 + l.off + l.h).unwrap_or(&lines[lines.len() - 1]);
                let y = (p.y - y0 - l.off).clamp(0.0, (l.h - 1.0).max(0.0));
                let b = l.byte_at(text, p.x - text_left + self.scroll_x, y);
                ed.set_caret(b, !pressed || shift);
                if !pressed && (p.y < text_rect.top() || p.y > text_rect.bottom()) {
                    self.sub += if p.y < text_rect.top() { -row_h } else { row_h };
                    self.follow = true;
                    ui.ctx().request_repaint();
                }
            }
        }

        // --- Painting.
        let text = ed.text();
        let caret = ed.caret();
        let caret_line = ed.line_of(caret);
        let sel = ed.selection().range();
        let gutter_painter = ui.painter_at(rect);
        let painter = ui.painter_at(text_rect);
        let origin_x = text_left - self.scroll_x;
        let look = MarkColours::of(&c);
        let band = |y: f32, (by, x0, x1): (f32, f32, f32), extra: f32| {
            Rect::from_min_max(pos2(origin_x + x0, y + by), pos2(origin_x + x1 + extra, y + by + row_h))
        };
        for l in &lines {
            let y = y0 + l.off;
            let number = if l.n == caret_line { c.gutter_active() } else { c.gutter() };
            gutter_painter.text(pos2(text_rect.left() - PAD, y), Align2::RIGHT_TOP, (l.n + 1).to_string(), font.clone(), number);
            match self.changes.marks.get(l.n).copied().flatten() {
                Some(crate::changes::Mark::Added) => {
                    gutter_painter.rect_filled(Rect::from_min_size(pos2(rect.left() + 1.0, y), vec2(3.0, l.h)), 0.0, c.diff_added());
                }
                Some(crate::changes::Mark::Modified) => {
                    gutter_painter.rect_filled(Rect::from_min_size(pos2(rect.left() + 1.0, y), vec2(3.0, l.h)), 0.0, c.diff_modified());
                }
                // Lines were taken out above: a small wedge on the line's top edge.
                Some(crate::changes::Mark::Removed) => {
                    let (x, w) = (rect.left() + 1.0, 6.0);
                    let tri = vec![pos2(x, y - 3.0), pos2(x + w, y), pos2(x, y + 3.0)];
                    gutter_painter.add(egui::Shape::convex_polygon(tri, c.diff_removed(), Stroke::NONE));
                }
                None => {}
            }
            let line = &text[l.start..l.end];
            let marks = marks(line, tree.and_then(|t| t.hard_break(l.n)));
            // The galley's character at the boundary before character `ch`.
            // The composition sits at a boundary: what starts there, or ends
            // after it, is drawn to its right.
            let ime = l.ime.map(|(at, len)| (pos::char_of_byte(line, at), len));
            let at = |ch: usize, end: bool| {
                ch + match ime {
                    Some((at, len)) if ch > at || (ch == at && !end) => len,
                    _ => 0,
                }
            };
            // Under the selection, so a selected break reads as selected.
            if let Some(r) = &marks.brk {
                for b in l.bands(at(r.start, false), at(r.end, true)) {
                    painter.rect_filled(band(y, b, 0.0), 2.0, look.fill);
                }
            }
            // The matches of the search, under the selection; the one the
            // selection is on is stronger.
            for f in &finds[finds.partition_point(|f| f.end <= l.start)..] {
                if f.start > l.end {
                    break;
                }
                let (s, e) = (f.start.clamp(l.start, l.end), f.end.clamp(l.start, l.end));
                let fill = if *f == sel { c.match_current_bg() } else { c.match_bg() };
                for b in l.bands(l.ch(text, s, false), l.ch(text, e, false)) {
                    if b.2 > b.1 {
                        painter.rect_filled(band(y, b, 0.0), 2.0, fill);
                    }
                }
            }
            if !sel.is_empty() && sel.start <= l.end && sel.end > l.start {
                let s = sel.start.clamp(l.start, l.end);
                let e = sel.end.clamp(l.start, l.end);
                let past_break = l.brk && sel.end > l.end;
                let bands = l.bands(l.ch(text, s, false), l.ch(text, e, false));
                let last = bands.len().saturating_sub(1);
                for (i, b) in bands.into_iter().enumerate() {
                    let extra = if past_break && i == last { char_w / 2.0 } else { 0.0 };
                    if b.2 + extra > b.1 {
                        painter.rect_filled(band(y, b, extra), 0.0, c.selection());
                    }
                }
            }
            painter.galley(pos2(origin_x, y), l.galley.clone(), c.fg);
            if let Some(r) = &marks.brk {
                for ch in r.clone() {
                    if let Some(&b) = l.bands(at(ch, false), at(ch + 1, true)).first() {
                        let rc = band(y, b, 0.0);
                        painter.circle_filled(pos2(rc.center().x, y + b.0 + row_h / 2.0), 1.25, look.dot);
                    }
                }
            }
            for &ch in &marks.wide {
                let Some(&b) = l.bands(at(ch, false), at(ch + 1, true)).first() else { continue };
                let (x0, x1) = (origin_x + b.1, origin_x + b.2);
                if x1 < text_rect.left() || x0 > text_rect.right() {
                    continue;
                }
                let side = ((x1 - x0).min(row_h) - 4.0).max(3.0);
                let cell = Rect::from_center_size(pos2((x0 + x1) / 2.0, y + b.0 + row_h / 2.0), vec2(side, side));
                painter.rect_stroke(cell, 1.5, Stroke::new(1.0, look.wide), StrokeKind::Inside);
            }
            if l.n == caret_line {
                let p = l.pos(text, caret, true);
                let bar = Rect::from_min_size(pos2(origin_x + p.x - 1.0, y + p.y), vec2(2.0, row_h));
                painter.rect_filled(bar, 0.0, if keys { c.caret() } else { c.caret_idle() });
                let p = l.pos(text, caret, false);
                self.caret_rect = Rect::from_min_size(pos2(origin_x + p.x, y + p.y), vec2(2.0, row_h));
            }
        }
        self.page = ((view_h / row_h).floor() as usize).saturating_sub(1).max(1);

        if let Some(t) = map {
            let look = ito_minimap::Look { row_h, font: font.clone() };
            let line = |i: usize| -> Vec<(String, Option<[u8; 3]>)> {
                let Some(r) = ed.line_range(i) else { return Vec::new() };
                let body = text[r.clone()].trim_end_matches(['\r', '\n']);
                let runs = t.lines.get(i).map_or(&[][..], |x| x.as_slice());
                crate::structure::pieces(body, runs, &c).into_iter().map(|(s, k)| (s.to_owned(), k)).collect()
            };
            let on_screen = lines.len().min(total);
            let shown = ito_minimap::show(ui, strip, text_rect, &t.rows, self.top, on_screen, &look, &line);
            if let Some(l) = shown.scroll_to {
                self.top = l;
                self.sub = 0.0;
                ui.ctx().request_repaint();
            }
        }
    }
}

/// One line's layout: its runs' colours, the gaps in the text's own colour,
/// and the composition (`ime`: where, what, how it looks) put in at the caret.
fn layout(
    line: &str,
    runs: Option<&crate::structure::Runs>,
    c: &ito_theme::Colors,
    plain: &TextFormat,
    ime: Option<(usize, &str, &TextFormat)>,
) -> LayoutJob {
    // The line cut into pieces of one colour each, in order.
    let mut pieces: Vec<(usize, usize, Color32)> = Vec::new();
    let mut at = 0;
    for (r, kind) in runs.into_iter().flatten() {
        let s = pos::floor(line, r.start.min(line.len())).max(at);
        let e = pos::floor(line, r.end.min(line.len()));
        if s >= e {
            continue;
        }
        if s > at {
            pieces.push((at, s, c.fg));
        }
        pieces.push((s, e, kind.color(c)));
        at = e;
    }
    if at < line.len() {
        pieces.push((at, line.len(), c.fg));
    }

    let mut job = LayoutJob::default();
    let format = |color: Color32| TextFormat { color, ..plain.clone() };
    let mut ime = ime;
    for (s, e, color) in pieces {
        if let Some((a, text, f)) = ime.filter(|(a, _, _)| *a >= s && *a < e) {
            if a > s {
                job.append(&line[s..a], 0.0, format(color));
            }
            job.append(text, 0.0, f.clone());
            job.append(&line[a..e], 0.0, format(color));
            ime = None;
        } else {
            job.append(&line[s..e], 0.0, format(color));
        }
    }
    if let Some((_, text, f)) = ime {
        job.append(text, 0.0, f.clone());
    }
    if job.sections.is_empty() {
        job.append("", 0.0, plain.clone());
    }
    job
}

/// What to mark on a line, in characters from its start.
#[derive(Debug, Default, PartialEq)]
struct Marks {
    /// The full-width spaces (U+3000).
    wide: Vec<usize>,
    /// The spaces (and tabs) at the end that make a hard break.
    brk: Option<Range<usize>>,
}

/// The marks of `line`.
///
/// The full-width spaces are found here, in the lines being painted only:
/// that is a byte search, never a parse, and always right for the text as it
/// is now, so a space typed this frame is marked this frame.
///
/// A hard break needs the parser, so `brk` is where the structure worker last
/// read one (bytes from the line's start). That reading may be older than the
/// line; it is kept only while its start still begins spaces and tabs that
/// run to the line's end (so it is on a character boundary), and then covers
/// all of them. Otherwise the line goes unmarked until the next reading.
fn marks(line: &str, brk: Option<Range<usize>>) -> Marks {
    let bytes = line.as_bytes();
    let blank = |b: &u8| *b == b' ' || *b == b'\t';
    let brk = brk.map(|r| r.start).filter(|&s| s < bytes.len() && bytes[s..].iter().all(blank)).map(|s| {
        let from = line[..s].chars().count();
        from..from + (bytes.len() - s)
    });
    let wide = if line.contains('\u{3000}') {
        line.chars().enumerate().filter(|&(_, c)| c == '\u{3000}').map(|(i, _)| i).collect()
    } else {
        Vec::new()
    };
    Marks { wide, brk }
}

/// The marks' colours: from the theme, a step quieter than the text, so they
/// are seen when looked for and not read as text.
struct MarkColours {
    /// A full-width space's thin box.
    wide: Color32,
    /// The fill under a hard break's spaces: the background with a little
    /// gold (the theme's colour for what waits for a person).
    fill: Color32,
    /// A dot on each of those spaces.
    dot: Color32,
}

impl MarkColours {
    fn of(c: &ito_theme::Colors) -> MarkColours {
        MarkColours { wide: c.invisible(), fill: mix(c.bg, c.warning(), 0.12), dot: mix(c.warning(), c.bg, 0.3) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structure::Kind;

    fn texts(job: &LayoutJob) -> Vec<&str> {
        job.sections.iter().map(|s| &job.text[s.byte_range.start.0..s.byte_range.end.0]).collect()
    }

    /// 9.5: A caret far down (`+5000`) is in view on the first frame, and
    /// stays in view when a face with another row height comes in (the fonts
    /// load after the first frame).
    #[test]
    fn a_far_caret_is_in_view_on_the_first_frame_and_after_the_face_changes() {
        let ctx = egui::Context::default();
        let input = || egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0))), ..Default::default() };
        let shown = |v: &View| {
            let top = 4999.0 * v.row_h;
            top >= v.scroll_y() && top + v.row_h <= v.scroll_y() + 600.0
        };
        let mut ed = Editor::new(&"line\n".repeat(6000));
        ed.go_to(5000, 1);
        let mut view = View::default();
        let mut frame =
            |view: &mut View| ctx.run_ui(input(), |ui| view.show(ui, &mut ed, None, &[], false, false)).drop_without_applying_deltas();
        frame(&mut view);
        assert!(shown(&view), "line 5000 at {} with the view at {}", 4999.0 * view.row_h, view.scroll_y());
        // Another face: egui's own proportional one, a little lower than its monospace.
        let before = view.row_h;
        let mut fonts = egui::FontDefinitions::default();
        let face = fonts.families[&egui::FontFamily::Proportional].clone();
        fonts.families.insert(egui::FontFamily::Monospace, face);
        ctx.set_fonts(fonts);
        frame(&mut view);
        assert_ne!(view.row_h, before, "the test needs a face of another height");
        assert!(shown(&view), "line 5000 at {} with the view at {}", 4999.0 * view.row_h, view.scroll_y());
    }

    /// 9.6: With word wrap a far caret is in view even when the lines above
    /// it take more than one row, and the first line stays a line of the text.
    #[test]
    fn a_wrapped_far_caret_is_in_view() {
        let ctx = egui::Context::default();
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(300.0, 400.0))), ..Default::default() };
        let mut ed = Editor::new(
            &format!(
                "{}
",
                "word ".repeat(40)
            )
            .repeat(300),
        );
        ed.go_to(200, 1);
        let mut view = View::default();
        for _ in 0..2 {
            ctx.run_ui(input.clone(), |ui| view.show(ui, &mut ed, None, &[], false, true)).drop_without_applying_deltas();
        }
        assert!(view.top_line() <= 199 && view.top_line() >= 150, "top {}", view.top_line());
        assert!(view.sub >= 0.0);
    }

    #[test]
    fn runs_from_an_older_reading_are_cut_to_the_line_and_its_characters() {
        let c = ito_theme::colors();
        let plain = TextFormat::simple(FontId::monospace(SIZE), c.fg);
        // "日本" is six bytes; a stale run ends inside the second character and
        // another reaches past the line's end.
        let runs = vec![(0..4, Kind::Heading), (4..40, Kind::Code)];
        let job = layout("日本x", Some(&runs), &c, &plain, None);
        assert_eq!(texts(&job), ["日", "本x"]);
        assert_eq!(job.sections[0].format.color, Kind::Heading.color(&c));
        let job = layout("", Some(&runs), &c, &plain, None);
        assert_eq!(job.sections.len(), 1, "an empty line still has a row");
    }

    #[test]
    fn the_composition_goes_in_at_the_caret() {
        let c = ito_theme::colors();
        let plain = TextFormat::simple(FontId::monospace(SIZE), c.fg);
        let ime = TextFormat { underline: Stroke::new(1.0, c.fg), ..plain.clone() };
        let job = layout("ab", None, &c, &plain, Some((1, "にほん", &ime)));
        assert_eq!(texts(&job), ["a", "にほん", "b"]);
        let job = layout("ab", None, &c, &plain, Some((2, "x", &ime)));
        assert_eq!(texts(&job), ["ab", "x"], "at the end of the line");
        let job = layout("", None, &c, &plain, Some((0, "x", &ime)));
        assert_eq!(texts(&job), ["x"]);
    }

    /// 8.10: A full-width space is marked where it is, by character, among
    /// characters of other widths; a hard break's spaces are marked from the
    /// reading, and a reading the text has moved away from never marks text.
    #[test]
    fn full_width_spaces_and_hard_breaks_are_marked_by_character() {
        assert_eq!(marks("", None), Marks::default());
        assert_eq!(marks("a\u{3000}b", None).wide, [1]);
        assert_eq!(marks("\u{3000}日本\u{3000}👍🏽\u{3000}", None).wide, [0, 3, 6], "the emoji and its tone are two characters");
        assert_eq!(marks("a b", None).wide, Vec::<usize>::new(), "a plain space is not marked");

        // The reading as it was made: "日本" is two characters, six bytes.
        assert_eq!(marks("日本  ", Some(6..8)).brk, Some(2..4));
        assert_eq!(marks("x\u{3000}  ", Some(4..6)), Marks { wide: vec![1], brk: Some(2..4) });
        // The text moved on since.
        assert_eq!(marks("日本   ", Some(6..8)).brk, Some(2..5), "a space typed at the end is in the run");
        assert_eq!(marks("日本  x", Some(6..8)).brk, None, "the line no longer ends in the spaces");
        assert_eq!(marks("日本", Some(6..8)).brk, None, "the spaces are gone");
        assert_eq!(marks("日", Some(6..8)).brk, None, "the line is shorter than the reading");
        assert_eq!(marks("本本本", Some(4..6)).brk, None, "the reading points inside a character");
        assert_eq!(marks("ab  ", Some(4..4)).brk, None, "nothing at the end");
    }

    /// The marks are quieter than the dim text and still seen, in every theme
    /// ito ships, light or dark; the fill is only a tint.
    #[test]
    fn the_marks_are_quiet_but_seen_in_every_theme() {
        for t in ito_theme::builtin() {
            let c = &t.colors;
            let look = MarkColours::of(c);
            let dim = ito_theme::contrast(c.dim, c.bg);
            let wide = ito_theme::contrast(look.wide, c.bg);
            assert!(wide > 2.0 && wide < dim, "{}: the box {wide:.2}, the dim text {dim:.2}", t.name);
            let fill = ito_theme::contrast(look.fill, c.bg);
            assert!(fill > 1.1 && fill < 1.5, "{}: the fill {fill:.2}", t.name);
            let dot = ito_theme::contrast(look.dot, look.fill);
            assert!(dot > 1.8 && dot < dim, "{}: the dots {dot:.2}, the dim text {dim:.2}", t.name);
        }
    }
}
