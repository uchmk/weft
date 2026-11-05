//! Big files (8 MiB and more): read-mostly. The file is not loaded; ito-lineindex
//! records where every line starts on a thread and the lines on screen are read
//! by seeking to them. Nothing is edited here: Ctrl+E hands the file to `$EDITOR`
//! at the line on top (docs/design.md, "大きなファイル").

use std::path::Path;
use std::sync::Arc;

use egui::{pos2, vec2, Align2, FontId, Rect, Sense, Ui};
use ito_lineindex::{Enc, Line, LineIndex};

/// A file this big (bytes) or bigger opens here.
pub const BIG_BYTES: u64 = 8 << 20;

/// The text's size (monospace), as in the editor's view.
const SIZE: f32 = 14.0;
const PAD: f32 = 8.0;
const BAR_W: f32 = 12.0;

/// Whether `path` is a file big enough to open as one of these.
pub fn is_big(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.len() >= BIG_BYTES)
}

pub struct Big {
    index: Arc<LineIndex>,
    /// The first line shown, from 0.
    top: usize,
    /// Scrolling that has not yet made a whole line.
    acc: f32,
    /// How many lines the last frame showed.
    rows: usize,
    /// The lines last read: (first line, rows, count the index had, lines).
    cache: Option<(usize, usize, usize, Vec<Line>)>,
}

impl Big {
    pub fn open(path: &Path) -> std::io::Result<Self> {
        Ok(Self { index: LineIndex::open(path)?, top: 0, acc: 0.0, rows: 30, cache: None })
    }

    /// The line on top, from 1 (what `$EDITOR` is opened at).
    pub fn line(&self) -> usize {
        self.top + 1
    }

    pub fn count(&self) -> usize {
        self.index.count()
    }

    /// How far the index has got, 0 to 1; 1 when it is done.
    pub fn progress(&self) -> f32 {
        if self.index.is_done() {
            1.0
        } else {
            self.index.progress()
        }
    }

    /// The file's encoding as the status bar names it.
    pub fn encoding(&self) -> String {
        match self.index.encoding() {
            Enc::Utf8 => "UTF-8".to_owned(),
            Enc::Utf16Le => "UTF-16LE".to_owned(),
            Enc::Utf16Be => "UTF-16BE".to_owned(),
            Enc::Legacy(e) => e.name().to_owned(),
        }
    }

    /// The file changed on the disk: what was appended is indexed, and a file cut or replaced is indexed anew.
    pub fn changed(&mut self) {
        if let ito_lineindex::Grow::Rebuild = self.index.grow() {
            if let Ok(i) = LineIndex::open(self.index.path()) {
                self.index = i;
            }
        }
        self.cache = None;
    }

    /// The lowest first line: the last line is at the foot of the window.
    fn last(&self) -> usize {
        self.count().saturating_sub(self.rows)
    }

    fn go(&mut self, line: usize) {
        self.top = line.min(self.last());
    }

    /// The keys that move through the file; they are taken before the keymap sees them.
    pub fn keys(&mut self, ctx: &egui::Context) {
        use egui::{Key, Modifiers};
        let page = self.rows.saturating_sub(1).max(1);
        ctx.input_mut(|i| {
            let mut take = |k: Key| i.consume_key(Modifiers::NONE, k) || i.consume_key(Modifiers::COMMAND, k);
            if take(Key::ArrowDown) {
                self.go(self.top.saturating_add(1));
            }
            if take(Key::ArrowUp) {
                self.go(self.top.saturating_sub(1));
            }
            if take(Key::PageDown) {
                self.go(self.top.saturating_add(page));
            }
            if take(Key::PageUp) {
                self.go(self.top.saturating_sub(page));
            }
            if take(Key::Home) {
                self.go(0);
            }
            if take(Key::End) {
                self.go(self.last());
            }
        });
    }

    pub fn show(&mut self, ui: &mut Ui) {
        let c = ito_theme::colors();
        let rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(rect, Sense::click_and_drag());
        let font = FontId::monospace(SIZE);
        let (row_h, char_w) = ui.ctx().fonts_mut(|f| (f.row_height(&font), f.glyph_width(&font, '0')));
        let total = self.count();
        let digits = total.max(1).to_string().len().max(3);
        let gutter = digits as f32 * char_w + 2.0 * PAD;
        let text_rect = Rect::from_min_max(pos2(rect.left() + gutter, rect.top()), pos2(rect.right() - BAR_W, rect.bottom()));
        let bar = Rect::from_min_max(pos2(rect.right() - BAR_W, rect.top()), rect.max);
        self.rows = ((rect.height() / row_h).floor() as usize).max(1);

        // The wheel, and the bar (a press or drag on it goes to that place in the file).
        if response.hovered() {
            self.acc -= ui.input(|i| i.smooth_scroll_delta.y);
            let whole = (self.acc / row_h).trunc();
            self.acc -= whole * row_h;
            if whole != 0.0 {
                self.go((self.top as f32 + whole).max(0.0) as usize);
            }
        }
        if let Some(p) = response.interact_pointer_pos().filter(|p| bar.x_range().contains(p.x) && response.is_pointer_button_down_on()) {
            let f = ((p.y - bar.top()) / bar.height()).clamp(0.0, 1.0);
            self.go((f * total as f32) as usize);
        }

        let wanted = (self.top, self.rows, total);
        if self.cache.as_ref().is_none_or(|(a, b, n, _)| (*a, *b, *n) != wanted) {
            let lines = self.index.lines(self.top..(self.top + self.rows).min(total));
            self.cache = Some((wanted.0, wanted.1, wanted.2, lines));
        }
        let painter = ui.painter_at(rect);
        let text_painter = ui.painter_at(text_rect);
        if let Some((first, _, _, lines)) = &self.cache {
            for (k, l) in lines.iter().enumerate() {
                let y = rect.top() + k as f32 * row_h;
                painter.text(pos2(text_rect.left() - PAD, y), Align2::RIGHT_TOP, (first + k + 1).to_string(), font.clone(), c.gutter());
                let shown = l.text.replace('\t', "    ");
                text_painter.text(pos2(text_rect.left(), y), Align2::LEFT_TOP, shown, font.clone(), c.fg);
                if l.cut {
                    // Where the line goes on (the rest is in the file, not here).
                    let w = text_rect.width();
                    text_painter.text(pos2(text_rect.left() + w - char_w, y), Align2::LEFT_TOP, "…", font.clone(), c.dim);
                }
            }
        }
        // The bar: where the lines on screen are in the file.
        if total > 0 {
            let h = bar.height();
            let thumb_h = (h * self.rows as f32 / total as f32).clamp(8.0, h);
            let y = bar.top() + (h - thumb_h) * (self.top as f32 / total.max(1) as f32);
            painter.rect_filled(Rect::from_min_size(pos2(bar.left() + 3.0, y), vec2(BAR_W - 6.0, thumb_h)), 2.0, c.gutter());
        }
        if !self.index.is_done() {
            ui.ctx().request_repaint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_of_8_mib_opens_by_line_and_the_view_stays_inside_it() {
        let dir = std::env::temp_dir().join(format!("weft-big-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let small = dir.join("small.md");
        std::fs::write(&small, "a\n").unwrap();
        assert!(!is_big(&small));
        let path = dir.join("big.md");
        let line = format!("{}\n", "x".repeat(79));
        std::fs::write(&path, line.repeat(BIG_BYTES as usize / 80 + 1)).unwrap();
        assert!(is_big(&path));
        let mut b = Big::open(&path).unwrap();
        while b.progress() < 1.0 {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let n = BIG_BYTES as usize / 80 + 1;
        assert_eq!(b.count(), n);
        b.rows = 30;
        b.go(usize::MAX);
        assert_eq!(b.line(), n - 30 + 1);
        b.go(0);
        assert_eq!(b.line(), 1);
        assert_eq!(b.encoding(), "UTF-8");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
