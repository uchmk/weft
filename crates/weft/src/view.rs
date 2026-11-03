//! The document on screen: the lines in view with their colours, the
//! selection, the caret, the input method's composition and the mouse.
//! Lines do not wrap yet: a long one scrolls sideways.
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
use egui::{pos2, vec2, Align2, Color32, Event, FontId, Galley, Id, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
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
}

impl Line {
    /// The x (from the galley's left) of `byte` in the text, on this line.
    fn x(&self, text: &str, byte: usize, with_ime: bool) -> f32 {
        let line = &text[self.start..self.end];
        let b = byte.clamp(self.start, self.end) - self.start;
        let mut ch = pos::char_of_byte(line, b);
        if let Some((at, len)) = self.ime {
            if with_ime && b >= at {
                ch += len;
            }
        }
        self.galley.pos_from_cursor(CCursor::new(ch)).min.x
    }

    /// The byte (in the text) nearest to `x` from the galley's left, on a
    /// grapheme's boundary.
    fn byte_at(&self, text: &str, x: f32, row_h: f32) -> usize {
        let line = &text[self.start..self.end];
        let mut ch = self.galley.cursor_from_pos(vec2(x, row_h / 2.0)).index.0;
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
}

pub struct View {
    /// How far the text is scrolled, in points (x to the left, y down).
    scroll: Vec2,
    /// How many lines a page moves: what fit last frame, less one.
    page: usize,
    /// Bring the caret into view next frame (it moved or the text changed).
    pub follow: bool,
    /// Where the caret was drawn, for the candidate window.
    caret_rect: Rect,
    ime: ito_ime::Ime,
}

impl Default for View {
    fn default() -> View {
        View { scroll: Vec2::ZERO, page: 20, follow: true, caret_rect: Rect::NOTHING, ime: ito_ime::Ime::new(Id::new("weft-editor")) }
    }
}

impl View {
    pub fn page(&self) -> usize {
        self.page
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

    /// Draw `ed` in what is left of `ui`. With `keys` the document has the
    /// keys: typing, the input method, copy and paste and the mouse go to it.
    pub fn show(&mut self, ui: &mut Ui, ed: &mut Editor, tree: Option<&Tree>, keys: bool, now_ms: u64) {
        let rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(rect, Sense::click_and_drag());
        let c = ito_theme::colors();
        let font = FontId::monospace(SIZE);
        let (row_h, char_w) = ui.ctx().fonts_mut(|f| (f.row_height(&font), f.glyph_width(&font, '0')));

        // --- What was typed.
        let mut preedit = String::new();
        if keys {
            let f = self.ime.frame(ui, self.caret_rect, ed.caret() as u64);
            let mut changed = false;
            for t in &f.commits {
                changed |= ed.commit(t, now_ms);
            }
            if f.composing {
                preedit = f.preedit;
                changed = true;
            }
            let events = ui.input(|i| i.events.clone());
            for e in events {
                match e {
                    Event::Text(t) => {
                        let t: String = t.chars().filter(|ch| !ch.is_control()).collect();
                        changed |= ed.insert(&t, now_ms);
                    }
                    Event::Paste(t) => changed |= ed.paste(&t, now_ms),
                    Event::Copy => {
                        if let Some(s) = ed.selected_text() {
                            ui.ctx().copy_text(s);
                        }
                    }
                    Event::Cut => {
                        if let Some(s) = ed.cut(now_ms) {
                            ui.ctx().copy_text(s);
                            changed = true;
                        }
                    }
                    _ => {}
                }
            }
            self.follow |= changed;
        }

        let text = ed.text();
        let total = ed.line_count();
        let digits = total.to_string().len().max(2);
        let gutter = digits as f32 * char_w + 2.0 * PAD;
        let text_rect = Rect::from_min_max(pos2(rect.left() + gutter, rect.top()), rect.max);
        let text_left = text_rect.left() + PAD;
        let view_w = (text_rect.width() - 2.0 * PAD).max(char_w);

        // --- Scrolling: the wheel, then the caret.
        if keys && response.hovered() {
            let (d, command) = ui.input(|i| (i.smooth_scroll_delta, i.modifiers.command));
            if !command {
                self.scroll -= d;
            }
        }
        let caret_line = ed.line_of(ed.caret());
        if self.follow {
            let top = caret_line as f32 * row_h;
            if top < self.scroll.y {
                self.scroll.y = top;
            } else if top + row_h > self.scroll.y + text_rect.height() {
                self.scroll.y = top + row_h - text_rect.height();
            }
        }
        self.scroll.y = self.scroll.y.clamp(0.0, (total - 1) as f32 * row_h);
        self.scroll.x = self.scroll.x.max(0.0);

        // --- The lines in view.
        let first = ((self.scroll.y / row_h).floor() as usize).min(total - 1);
        let y0 = text_rect.top() - (self.scroll.y - first as f32 * row_h);
        let count = (text_rect.height() / row_h).ceil() as usize + 1;
        let plain = TextFormat::simple(font.clone(), c.fg);
        let ime_format = TextFormat { background: c.selection(), underline: Stroke::new(1.0, c.fg), ..plain.clone() };
        let mut lines: Vec<Line> = Vec::with_capacity(count);
        let mut start = ed.line_range(first).map_or(text.len(), |r| r.start);
        for n in first..(first + count).min(total) {
            let (end, brk, next) = match text[start..].find('\n') {
                Some(i) => {
                    let nl = start + i;
                    let end = if nl > start && text.as_bytes()[nl - 1] == b'\r' { nl - 1 } else { nl };
                    (end, true, nl + 1)
                }
                None => (text.len(), false, text.len()),
            };
            let line = &text[start..end];
            let ime_at = (n == caret_line && !preedit.is_empty()).then(|| ed.caret().clamp(start, end) - start);
            let job = layout(line, tree.and_then(|t| t.lines.get(n)), &c, &plain, ime_at.map(|a| (a, preedit.as_str(), &ime_format)));
            let galley = ui.painter().layout_job(job);
            let ime = ime_at.map(|a| (a, preedit.chars().count()));
            lines.push(Line { n, start, end, brk, galley, ime });
            start = next;
            if !brk {
                break;
            }
        }

        // Sideways, now that the caret's line is laid out.
        if self.follow {
            if let Some(l) = lines.iter().find(|l| l.n == caret_line) {
                let x = l.x(text, ed.caret(), true);
                if x < self.scroll.x {
                    self.scroll.x = (x - 4.0 * char_w).max(0.0);
                } else if x + char_w > self.scroll.x + view_w {
                    self.scroll.x = x + 4.0 * char_w - view_w;
                }
            }
            self.follow = false;
        }

        // --- The mouse: a click puts the caret, a drag (or Shift+click) selects.
        if keys && !lines.is_empty() && response.is_pointer_button_down_on() {
            let (pressed, primary, shift) = ui.input(|i| (i.pointer.primary_pressed(), i.pointer.primary_down(), i.modifiers.shift));
            if let (true, Some(p)) = (primary, response.interact_pointer_pos()) {
                let row = ((p.y - y0) / row_h).floor().max(0.0) as usize;
                let l = &lines[row.min(lines.len() - 1)];
                let b = l.byte_at(text, p.x - text_left + self.scroll.x, row_h);
                ed.set_caret(b, !pressed || shift);
                if !pressed && (p.y < text_rect.top() || p.y > text_rect.bottom()) {
                    self.scroll.y += if p.y < text_rect.top() { -row_h } else { row_h };
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
        let origin_x = text_left - self.scroll.x;
        let look = MarkColours::of(&c);
        for (i, l) in lines.iter().enumerate() {
            let y = y0 + i as f32 * row_h;
            let number = if l.n == caret_line { c.fg } else { c.dim };
            gutter_painter.text(pos2(text_rect.left() - PAD, y), Align2::RIGHT_TOP, (l.n + 1).to_string(), font.clone(), number);
            let line = &text[l.start..l.end];
            let marks = marks(line, tree.and_then(|t| t.hard_break(l.n)));
            // The x of the boundary before character `ch`. The composition
            // sits at a boundary: what starts there, or ends after it, is
            // drawn to its right.
            let ime = l.ime.map(|(at, len)| (pos::char_of_byte(line, at), len));
            let gx = |ch: usize, end: bool| {
                let shift = match ime {
                    Some((at, len)) if ch > at || (ch == at && !end) => len,
                    _ => 0,
                };
                origin_x + l.galley.pos_from_cursor(CCursor::new(ch + shift)).min.x
            };
            // Under the selection, so a selected break reads as selected.
            if let Some(r) = &marks.brk {
                let (x0, x1) = (gx(r.start, false), gx(r.end, true));
                painter.rect_filled(Rect::from_min_max(pos2(x0, y), pos2(x1, y + row_h)), 2.0, look.fill);
            }
            if !sel.is_empty() && sel.start <= l.end && sel.end > l.start {
                let s = sel.start.clamp(l.start, l.end);
                let e = sel.end.clamp(l.start, l.end);
                let past_break = l.brk && sel.end > l.end;
                let x0 = origin_x + l.x(text, s, false);
                let x1 = origin_x + l.x(text, e, false) + if past_break { char_w / 2.0 } else { 0.0 };
                if x1 > x0 {
                    painter.rect_filled(Rect::from_min_max(pos2(x0, y), pos2(x1, y + row_h)), 0.0, c.selection());
                }
            }
            painter.galley(pos2(origin_x, y), l.galley.clone(), c.fg);
            if let Some(r) = &marks.brk {
                for ch in r.clone() {
                    let x = (gx(ch, false) + gx(ch + 1, true)) / 2.0;
                    painter.circle_filled(pos2(x, y + row_h / 2.0), 1.25, look.dot);
                }
            }
            for &ch in &marks.wide {
                let (x0, x1) = (gx(ch, false), gx(ch + 1, true));
                if x1 < text_rect.left() || x0 > text_rect.right() {
                    continue;
                }
                let side = ((x1 - x0).min(row_h) - 4.0).max(3.0);
                let cell = Rect::from_center_size(pos2((x0 + x1) / 2.0, y + row_h / 2.0), vec2(side, side));
                painter.rect_stroke(cell, 1.5, Stroke::new(1.0, look.wide), StrokeKind::Inside);
            }
            if l.n == caret_line {
                let x = origin_x + l.x(text, caret, true);
                let bar = Rect::from_min_size(pos2(x - 1.0, y), vec2(2.0, row_h));
                painter.rect_filled(bar, 0.0, if keys { c.fg } else { c.dim });
                let at = origin_x + l.x(text, caret, false);
                self.caret_rect = Rect::from_min_size(pos2(at, y), vec2(2.0, row_h));
            }
        }
        self.page = ((text_rect.height() / row_h).floor() as usize).saturating_sub(1).max(1);
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
        MarkColours { wide: mix(c.dim, c.bg, 0.4), fill: mix(c.bg, c.wait, 0.12), dot: mix(c.wait, c.bg, 0.3) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structure::Kind;

    fn texts(job: &LayoutJob) -> Vec<&str> {
        job.sections.iter().map(|s| &job.text[s.byte_range.start.0..s.byte_range.end.0]).collect()
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
