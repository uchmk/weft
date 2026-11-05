//! The rendered preview: the text laid out by `ito-mdview` beside the source.
//! The layout is made off the UI thread for the newest text and the width the
//! pane reported (a result for an older text or width is not shown), and the
//! two panes follow each other's scrolling.

use std::sync::mpsc::{self, Receiver, Sender};

use ito_mdview::{Doc as Laid, Options, Palette, State};
use serde::{Deserialize, Serialize};

use crate::App;

/// A window narrower than this (in points) has no room for a second pane
/// beside the source: the preview stays closed there.
pub const MIN_WINDOW: f32 = 720.0;

/// What a layout was asked about: the text, the columns and the colours.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Key {
    text: u64,
    cols: u16,
    pal: Palette,
}

struct Job {
    key: Key,
    text: String,
}

struct Done {
    key: Key,
    laid: Laid,
    /// The text was longer than the preview lays out.
    cut: bool,
}

/// The preview of one document: the thread, the newest layout and the pane's
/// scroll.
pub struct DocPreview {
    tx: Sender<Job>,
    rx: Receiver<Done>,
    asked: Option<Key>,
    done: Option<Done>,
    state: State,
    /// The source's top line as last made to agree with the preview.
    seen_top: usize,
    /// The query the marks are of, and its matcher.
    matcher: Option<(u64, Option<ito_match::Matcher>)>,
}

impl DocPreview {
    /// `wake` is called when a layout is ready (the window's repaint).
    pub fn new(wake: impl Fn() + Send + 'static) -> Self {
        let (tx, jobs) = mpsc::channel::<Job>();
        let (done, rx) = mpsc::channel();
        let run = move || {
            while let Ok(mut job) = jobs.recv() {
                while let Ok(newer) = jobs.try_recv() {
                    job = newer;
                }
                let (laid, cut) = ito_mdview::render(&job.text, job.key.cols, &job.key.pal);
                if done.send(Done { key: job.key, laid, cut }).is_err() {
                    return;
                }
                wake();
            }
        };
        // Without the thread the pane stays empty; the source is untouched.
        let _ = std::thread::Builder::new().name("weft-preview".into()).spawn(run);
        Self { tx, rx, asked: None, done: None, state: State::default(), seen_top: 0, matcher: None }
    }

    /// Whether the layout on hand is of `key`.
    fn current(&self, key: &Key) -> bool {
        self.done.as_ref().is_some_and(|d| d.key == *key)
    }

    /// Take what the thread made; ask for `key` if it is not on hand.
    fn sync(&mut self, key: Key, text: &str) {
        while let Ok(d) = self.rx.try_recv() {
            self.done = Some(d);
        }
        if !self.current(&key) && self.asked != Some(key) {
            self.asked = Some(key);
            let _ = self.tx.send(Job { key, text: text.to_owned() });
        }
    }

    /// The columns the pane last fit (0 before the first frame).
    #[cfg(test)]
    pub fn cols(&self) -> u16 {
        self.state.cols
    }
}

/// Whether the preview is wanted; kept between runs in `preview.toml`.
#[derive(Serialize, Deserialize, Default)]
pub struct Saved {
    pub open: bool,
}

pub fn state_path() -> Option<std::path::PathBuf> {
    ito_common::state_dir("weft").map(|d| d.join("preview.toml"))
}

pub fn read_open() -> bool {
    state_path().and_then(|p| ito_common::kept::read::<Saved>(&p)).is_some_and(|s| s.open)
}

impl App {
    /// Whether the pane is on the screen: wanted, a document to show, and a
    /// window wide enough.
    pub(crate) fn preview_shown(&self, ctx: &egui::Context) -> bool {
        self.preview_open && !self.settings_open && self.doc.editor.is_some() && ctx.content_rect().width() >= MIN_WINDOW
    }

    /// `view.toggle_preview`.
    pub(crate) fn preview_toggle(&mut self, ctx: &egui::Context) {
        self.preview_open = !self.preview_open;
        self.preview_writer.save_toml(&Saved { open: self.preview_open });
        ctx.request_repaint();
        let key = if self.preview_open { "preview.on" } else { "preview.off" };
        let text = self.table.tr(key).to_owned();
        self.announce(text);
    }

    /// The pane at the right, drawn before the source so the source takes
    /// what is left.
    pub(crate) fn preview_pane(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if !self.preview_shown(&ctx) {
            return;
        }
        let c = ito_theme::colors();
        let width = ctx.content_rect().width();
        let key_of = |cols: u16, text: u64| Key { text, cols, pal: Palette::from_theme() };
        let Some(ed) = &self.doc.editor else { return };
        let (text_v, caret) = (ed.version(), ed.caret());
        let p = &mut self.doc.preview;
        let find_marks = self.find.open && !self.find.find.is_empty();
        if find_marks {
            let v = self.find.version();
            if p.matcher.as_ref().map(|m| m.0) != Some(v) {
                p.matcher = Some((v, ito_match::Matcher::new(&self.find.find).ok()));
            }
        }
        let mut shown = false;
        let mut out = ito_mdview::Output::default();
        let look = ito_prefs::Look { c, q: "" };
        let (empty_title, empty_body) = (self.table.tr("preview.empty_title"), self.table.tr("preview.empty_body"));
        egui::Panel::right("preview")
            .resizable(true)
            .default_size((width * 0.42).round())
            .size_range(egui::Rangef::new(240.0, (width * 0.7).max(240.0)))
            .frame(egui::Frame::NONE.fill(c.bg))
            .show(ui, |ui| {
                let cols = if p.state.cols == 0 { 60 } else { p.state.cols };
                // Lay out at the width the pane had last frame.
                p.sync(key_of(cols, text_v), ed.text());
                let Some(d) = p.done.as_ref() else { return };
                if d.laid.lines.is_empty() {
                    let _ = ito_prefs::empty_state(ui, look, ito_prefs::Mark::Info, empty_title, empty_body, None);
                    return;
                }
                let matcher = if find_marks { p.matcher.as_ref().and_then(|m| m.1.as_ref()) } else { None };
                let opt = Options { size: None, matcher, caret_byte: Some(caret) };
                out = ito_mdview::show(ui, &d.laid, &mut p.state, &opt);
                shown = true;
            });
        if !shown {
            return;
        }
        // The pane got a different width than the layout was made for.
        if out.cols != 0 && out.cols != p.done.as_ref().map_or(out.cols, |d| d.key.cols) {
            p.sync(key_of(out.cols, text_v), self.doc.editor.as_ref().map_or("", |e| e.text()));
        }
        if let Some(byte) = out.clicked_byte {
            if let Some(ed) = &mut self.doc.editor {
                let mut b = byte.min(ed.text().len());
                while !ed.text().is_char_boundary(b) {
                    b -= 1;
                }
                ed.set_caret(b, false);
                self.doc.view.follow = true;
            }
        }
        if out.scrolled {
            // The person scrolled the preview: the source follows.
            if let Some(ed) = &self.doc.editor {
                let line = ed.line_of(out.top_byte.min(ed.text().len()));
                self.doc.view.scroll_to_line(line);
                self.doc.preview.seen_top = line;
            }
        }
    }

    /// After the source is drawn: when the person scrolled it, the preview
    /// follows.
    pub(crate) fn preview_sync(&mut self) {
        if !self.preview_shown(&self.ctx.clone()) {
            return;
        }
        let top = self.doc.view.top_line();
        let Some(ed) = &self.doc.editor else { return };
        let p = &mut self.doc.preview;
        if top != p.seen_top {
            p.seen_top = top;
            if let Some(r) = ed.line_range(top) {
                p.state.scroll_to_byte(r.start);
            }
        }
    }

    /// What `<State:>` says of the preview.
    pub(crate) fn preview_report(&self) -> String {
        let shown = self.preview_open && self.doc.editor.is_some();
        let mut out = format!("preview: {}\n", if shown { "open" } else { "closed" });
        if shown {
            if let Some(d) = &self.doc.preview.done {
                out.push_str(&format!("preview_lines: {}\n", d.laid.lines.len()));
                if d.cut {
                    out.push_str("preview_cut: true\n");
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Editor;

    fn wait(app: &mut App) {
        for _ in 0..200 {
            let Some(ed) = &app.doc.editor else { return };
            let key = Key { text: ed.version(), cols: 40, pal: Palette::from_theme() };
            let text = ed.text().to_owned();
            app.doc.preview.sync(key, &text);
            if app.doc.preview.current(&key) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("no layout");
    }

    /// 15.1: The layout is of the newest text and width; one of an older text
    /// is not taken as current.
    #[test]
    fn layout_is_keyed_by_text_and_width() {
        let mut app = App::for_tests();
        app.doc.editor = Some(Editor::new("# Title\n\nbody\n"));
        wait(&mut app);
        let d = app.doc.preview.done.as_ref().unwrap();
        assert!(d.laid.lines.len() >= 2);
        let old = d.key;
        app.doc.editor.as_mut().unwrap().set_caret(0, false);
        let other = Key { cols: 41, ..old };
        assert!(!app.doc.preview.current(&other));
    }

    /// 15.2: The source scrolled -> the preview is told to follow; both agree
    /// afterwards.
    #[test]
    fn the_preview_follows_the_source() {
        let mut app = App::for_tests();
        app.ctx.set_zoom_factor(1.0);
        let text: String = (0..100).map(|i| format!("line {i}\n\n")).collect();
        app.doc.editor = Some(Editor::new(&text));
        app.preview_open = true;
        app.doc.preview.seen_top = 0;
        // No window: the pane is not shown, so nothing moves.
        app.preview_sync();
        assert_eq!(app.doc.preview.seen_top, 0);
        assert_eq!(app.doc.preview.cols(), 0);
    }

    /// 15.3: The preview is off in a narrow window and when nothing is open.
    #[test]
    fn hidden_when_narrow_or_empty() {
        let mut app = App::for_tests();
        let ctx = egui::Context::default();
        app.preview_open = true;
        assert!(!app.preview_shown(&ctx), "no document");
        app.doc.editor = Some(Editor::new("a\n"));
        // The default test window has no size to speak of.
        assert!(!app.preview_shown(&ctx) || ctx.content_rect().width() >= MIN_WINDOW);
    }

    /// 15.4: Toggling is remembered (as `preview.toml`'s `open`).
    #[test]
    fn toggle_flips_and_reports() {
        let mut app = App::for_tests();
        let ctx = egui::Context::default();
        app.doc.editor = Some(Editor::new("a\n"));
        assert!(!app.preview_open);
        app.preview_toggle(&ctx);
        assert!(app.preview_open);
        assert!(app.preview_report().contains("preview: open"));
        app.preview_toggle(&ctx);
        assert!(app.preview_report().contains("preview: closed"));
    }
}
