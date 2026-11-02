//! The document on screen: the lines in view with their colours, the
//! selection, the caret, the input method's composition and the mouse.
//! Lines do not wrap yet: a long one scrolls sideways.
//!
//! Only the lines in view are laid out, from the colours the structure worker
//! last read. Those may be a version behind the text; a run is cut to the
//! line and to a character boundary, so old colours never split a character.

use std::sync::Arc;

use egui::text::{CCursor, LayoutJob, TextFormat};
use egui::{pos2, vec2, Align2, Color32, Event, FontId, Galley, Id, Rect, Sense, Stroke, Ui, Vec2};
use ito_textbuf::pos;

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
                        if let Some(s) = ed.cut() {
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
        let total = pos::line_count(text);
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
        let caret_line = pos::line_of(text, ed.caret());
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
        let mut start = pos::line_range(text, first).map_or(text.len(), |r| r.start);
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
        let caret_line = pos::line_of(text, caret);
        let sel = ed.selection().range();
        let gutter_painter = ui.painter_at(rect);
        let painter = ui.painter_at(text_rect);
        let origin_x = text_left - self.scroll.x;
        for (i, l) in lines.iter().enumerate() {
            let y = y0 + i as f32 * row_h;
            let number = if l.n == caret_line { c.fg } else { c.dim };
            gutter_painter.text(pos2(text_rect.left() - PAD, y), Align2::RIGHT_TOP, (l.n + 1).to_string(), font.clone(), number);
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
}
