//! weft: Markdown 専用エディター。キーだけで見出し・リスト・表などの構造を組み替え、日本語 IME に強く、フォルダー（vault）も扱う。kura の .md の opener
//!
//! What is wired here: the window (its place kept, its own title bar with the
//! bell and the notice list), common.toml's theme, language and scale (and
//! the settings screen that writes them, Ctrl+,), `--version` / `--help`,
//! F12 (the bug report), `--keys` (a script that drives the window for a
//! test lane), keymap.toml (Ctrl+Q quits) and the key help (F1).
//!
//! The document: `weft FILE` opens it in the Document Engine
//! ([`engine`], ito-textbuf), [`view`] draws it with the colours the
//! [`structure`] worker last read, and the keys run the named commands of
//! [`commands`]. Saving and `$EDITOR` run on a thread of their own, in order.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod commands;
mod engine;
mod envreport;
mod help;
mod i18n;
mod icon;
mod keymap;
mod notify;
mod prefs;
mod settings;
mod structure;
mod view;

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender};

use ito_common::window::{Saved, Tracker};
use ito_keymap::Match;
use ito_keys::Key;
use ito_keyscript::{Do, Runner, Step};

/// The window's icon, in pixels square.
const ICON_PX: u32 = 128;

const REPO: &str = "https://github.com/uchmk/weft";

const USAGE: &str = "\
weft — Markdown 専用エディター。キーだけで見出し・リスト・表などの構造を組み替え、日本語 IME に強く、フォルダー（vault）も扱う。kura の .md の opener

usage: weft [+LINE[:COL]] [FILE] [--keys SCRIPT]
       weft --version | --help | env

  FILE           the Markdown file to edit (one that is not there yet is
                 made when it is saved)
  +LINE[:COL]    start with the caret there, both from 1, the column in
                 characters (kura's opener passes it); before or after FILE.
                 Past the end is the last line, or the line's end. A FILE
                 whose name starts with + is given as ./+name
  env            print what a bug report needs (version, files, last run); no window
  --keys SCRIPT  press these keys once the window is up, as kura's --keys;
                 WEFT_KEYS_DONE names the file that says how it ended
";

/// The sizes of the screen: body 13, buttons and status 12, small 11, the big heading 18;
/// a button's padding is 10 by 3 (as the other uchmk apps).
fn type_scale(style: &mut egui::Style) {
    use egui::{
        FontFamily::{Monospace, Proportional},
        FontId, TextStyle,
    };
    style.text_styles.insert(TextStyle::Body, FontId::new(13.0, Proportional));
    style.text_styles.insert(TextStyle::Button, FontId::new(12.0, Proportional));
    style.text_styles.insert(TextStyle::Small, FontId::new(11.0, Proportional));
    style.text_styles.insert(TextStyle::Heading, FontId::new(18.0, Proportional));
    style.text_styles.insert(TextStyle::Monospace, FontId::new(12.0, Monospace));
    style.spacing.button_padding = egui::vec2(10.0, 3.0);
}

/// What the file thread does, in the order it was asked.
enum FileJob {
    Save(engine::SaveJob),
    /// `$EDITOR` at the caret. After a save that failed, it is not run: the
    /// editor would open what is on the disk, not what the window shows.
    Open {
        // Boxed: far larger than a save job on some targets.
        cfg: Box<ito_openin::Config>,
        path: PathBuf,
        at: ito_openin::Pos,
    },
}

/// What came back from the file thread.
enum FileDone {
    Saved { version: u64, path: PathBuf, result: Result<(), String> },
    OpenFailed(String),
}

/// The thread that writes the files and starts `$EDITOR`, off the UI thread
/// and one job at a time (a save is on the disk before the editor reads it).
fn file_thread(ctx: egui::Context) -> (Sender<FileJob>, Receiver<FileDone>) {
    let (tx, jobs) = std::sync::mpsc::channel::<FileJob>();
    let (done, rx) = std::sync::mpsc::channel();
    let run = move || {
        let mut failed = false;
        while let Ok(job) = jobs.recv() {
            let out = match job {
                FileJob::Save(job) => {
                    let result = engine::write(&job);
                    failed = result.is_err();
                    FileDone::Saved { version: job.version, path: job.path, result }
                }
                FileJob::Open { cfg, path, at } => {
                    // The failed save was told of already.
                    if std::mem::take(&mut failed) {
                        continue;
                    }
                    match cfg.open_file_at(&path, Some(at)) {
                        Ok(_) => continue,
                        Err(e) => FileDone::OpenFailed(e),
                    }
                }
            };
            if done.send(out).is_err() {
                return;
            }
            ctx.request_repaint();
        }
    };
    if let Err(e) = std::thread::Builder::new().name("weft-files".into()).spawn(run) {
        eprintln!("weft: no file thread: {e}");
    }
    (tx, rx)
}

struct App {
    table: ito_i18n::Table,
    /// common.toml, the themes, keymap.toml and config.toml's `[notify]`,
    /// kept current by a watcher; the settings screen's state.
    prefs: prefs::Prefs,
    settings_open: bool,
    /// The key help (F1) while it is open.
    help: help::Help,
    tracker: Option<Tracker>,
    lang: String,
    /// The last thing done that the status bar tells of (a save, a scale change).
    note: Option<String>,
    /// The document, when one is open (`weft FILE`).
    editor: Option<engine::Editor>,
    view: view::View,
    /// The newest reading of the structure, and the thread that reads it.
    tree: Option<structure::Tree>,
    worker: structure::Worker,
    /// The file thread's way in and out, and how many saves are on it.
    files: Sender<FileJob>,
    done: Receiver<FileDone>,
    saving: usize,
    /// When the window opened: typing's clock (keys close together are one undo step).
    started: std::time::Instant,
    /// What the bell lists, oldest first. `go` is the file a notice is
    /// about: its row, or a click on the system's notification for it, puts
    /// that document in front when it is the one open.
    notices: Vec<ito_notices::Notice<PathBuf>>,
    next_notice: u64,
    /// The list under the bell is open, and where the bell was last drawn.
    bell_open: bool,
    bell_at: egui::Rect,
    /// The command palette while it is open (Ctrl+Shift+P, or the bar's box).
    palette: Option<ito_palette::View>,
    /// The Japanese face, read off the UI thread; `None` once it is registered.
    font: Option<std::sync::mpsc::Receiver<Option<Vec<u8>>>>,
    /// The `--keys` script while it plays, and what its steps ask the frame to do.
    script: Option<Runner>,
    todo: Vec<Do>,
    /// The keys in force (defaults/keymap.toml under the person's own), the
    /// chord typed so far, and what the person's files got wrong (under the
    /// bell, once).
    keys: keymap::Keys,
    key_pending: Vec<Key>,
    key_notes: Vec<String>,
    /// What each of the person's files warned of when last heard: a warning
    /// goes under the bell once, and again only after it went away.
    warned: Warned,
    /// What the screen reader is to hear at the end of this frame.
    said: Vec<String>,
    /// Quitting, once asked to (`None` while nobody has).
    quit: Option<Quit>,
}

/// The warnings last put under the bell, file by file (common.toml, the
/// themes folder, the theme common.toml picks, config.toml, keymap.toml).
#[derive(Default)]
struct Warned {
    common: Vec<String>,
    themes: Vec<String>,
    theme: Vec<String>,
    config: Vec<String>,
    keymap: Vec<String>,
}

/// The warnings in `now` that were not in `was`; `was` becomes `now`. A file
/// read again says what it said before, and the theme is picked every frame:
/// none of that is said twice.
fn news(was: &mut Vec<String>, now: &[String]) -> Vec<String> {
    let new = now.iter().filter(|w| !was.contains(w)).cloned().collect();
    if was.as_slice() != now {
        *was = now.to_vec();
    }
    new
}

/// Where quitting is. With changes not saved, the window asks first (save
/// and quit, quit without saving, or stay); a save asked for there closes the
/// window once it lands, and one that fails leaves it open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Quit {
    Asking,
    Saving,
    Go,
}

/// What the palette can do: the two fixed keys' commands, and every command
/// of [`commands::palette`] by name (as a key runs it).
#[derive(Clone, Copy)]
enum Command {
    ReportBug,
    ResetScale,
    Run(&'static str),
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, saved: &Saved, script: Option<Runner>, file: Option<PathBuf>) -> Self {
        saved.put(cc);
        // What this run drew with, for the bug report and `weft env`.
        let mut used = ito_common::runinfo::RunInfo::new(env!("CARGO_PKG_VERSION"));
        if let Some(rs) = cc.wgpu_render_state.as_ref() {
            let info = rs.adapter.get_info();
            used.adapter = info.name;
            used.backend = format!("{:?}", info.backend);
            used.device = format!("{:?}", info.device_type);
        }
        used.ppp = cc.egui_ctx.pixels_per_point();
        ito_common::runinfo::save("weft", &used);
        let mut prefs = prefs::Prefs::load(ito_common::base_dir());
        let lang = i18n::resolve(prefs.common.language.as_deref());
        // The lang files' errors; what the other files got wrong is heard below.
        let (table, key_notes) = i18n::table(&lang);
        // keymap.toml as read now makes the keys, as a later read does.
        prefs.keymap_new.get_or_insert(None);
        prefs.watch(&cc.egui_ctx);
        prefs.apply_theme(&cc.egui_ctx);
        cc.egui_ctx.set_visuals(ito_theme::colors().visuals());
        cc.egui_ctx.all_styles_mut(type_scale);
        // The scale is common.toml's, so egui's own Ctrl+= / Ctrl+- stay out of it.
        cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
        cc.egui_ctx.set_zoom_factor(prefs.common.scale_or_one());
        let wake = cc.egui_ctx.clone();
        let (files, done) = file_thread(cc.egui_ctx.clone());
        // A click on a system notification weft showed brings this window forward.
        notify::set_window(&cc.egui_ctx);
        let mut app = Self {
            table,
            prefs,
            settings_open: false,
            help: help::Help::default(),
            tracker: saved.tracker(),
            lang,
            note: None,
            editor: None,
            view: view::View::default(),
            tree: None,
            worker: structure::Worker::new(move || wake.request_repaint()),
            files,
            done,
            saving: 0,
            started: std::time::Instant::now(),
            notices: Vec::new(),
            next_notice: 0,
            bell_open: false,
            bell_at: egui::Rect::NOTHING,
            palette: None,
            font: Some(font_reader(&cc.egui_ctx)),
            script,
            todo: Vec::new(),
            keys: keymap::Keys::load(None).0,
            key_pending: Vec::new(),
            key_notes,
            warned: Warned::default(),
            said: Vec::new(),
            quit: None,
        };
        // What the person's files got wrong goes under the bell, once.
        app.take_keymap();
        app.hear_files();
        for w in &app.key_notes {
            eprintln!("weft: {w}");
        }
        if let Some(path) = file {
            app.open(&path);
        }
        app
    }

    /// Open `path` as the document. One that cannot be read leaves the
    /// window empty, and one that is not UTF-8 opens read-only; both are told
    /// of under the bell.
    fn open(&mut self, path: &Path) {
        let name = path.display().to_string();
        match engine::Editor::open(path) {
            Ok(ed) => {
                if ed.locked().is_some() {
                    let title = self.table.trf("file.locked", &[&name]);
                    self.outcome(ito_notices::Kind::Warn, title, Some(path.to_path_buf()));
                }
                self.editor = Some(ed);
                self.tree = None;
                self.view = view::View::default();
            }
            Err(e) => {
                let title = self.table.trf("file.open_failed", &[&name, &e]);
                self.outcome(ito_notices::Kind::Err, title, None);
            }
        }
    }

    /// An app with nothing read and no window (the tests).
    #[cfg(test)]
    fn for_tests() -> Self {
        let (table, _) = i18n::table("en");
        let (keys, _) = keymap::Keys::load(None);
        let (files, done) = file_thread(egui::Context::default());
        Self {
            table,
            prefs: prefs::Prefs::empty(),
            settings_open: false,
            help: help::Help::default(),
            tracker: None,
            lang: "en".to_owned(),
            note: None,
            editor: None,
            view: view::View::default(),
            tree: None,
            worker: structure::Worker::new(|| {}),
            files,
            done,
            saving: 0,
            started: std::time::Instant::now(),
            notices: Vec::new(),
            next_notice: 0,
            bell_open: false,
            bell_at: egui::Rect::NOTHING,
            palette: None,
            font: None,
            script: None,
            todo: Vec::new(),
            keys,
            key_pending: Vec::new(),
            key_notes: Vec::new(),
            warned: Warned::default(),
            said: Vec::new(),
            quit: None,
        }
    }

    /// The document has the keys: one is open and nothing covers it (the
    /// settings, the help, the palette, the list under the bell, the
    /// question before quitting).
    fn editing(&self) -> bool {
        self.editor.is_some() && !self.settings_open && !self.help.open && self.palette.is_none() && !self.bell_open && self.quit.is_none()
    }

    /// The document has changes not yet saved.
    fn dirty(&self) -> bool {
        self.editor.as_ref().is_some_and(engine::Editor::is_dirty)
    }

    /// Ctrl+Q or the window's close: straight away when nothing would be
    /// lost, else the question first.
    fn quit(&mut self, ctx: &egui::Context) {
        if self.quit == Some(Quit::Go) {
            return;
        }
        if self.dirty() {
            self.help.close();
            self.palette = None;
            self.bell_open = false;
            if self.quit != Some(Quit::Asking) {
                // The question has no focus of its own to be read out by.
                self.announce(self.quit_spoken());
            }
            self.quit = Some(Quit::Asking);
        } else {
            self.quit = Some(Quit::Go);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    /// The question's answer: 0 stay, 1 quit without saving, 2 save and quit.
    fn answer_quit(&mut self, ctx: &egui::Context, pick: usize) {
        match pick {
            0 => self.quit = None,
            1 => {
                self.quit = Some(Quit::Go);
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            _ => {
                self.quit = Some(Quit::Saving);
                self.save();
            }
        }
    }

    /// The question's keys, read before anything else sees them: Esc stays,
    /// `n` quits without saving, Enter (or `y`) saves and quits.
    fn quit_keys(&mut self, ctx: &egui::Context) {
        use egui::{Key as K, Modifiers as M};
        let pick = ctx.input_mut(|i| {
            if i.consume_key(M::NONE, K::Escape) {
                Some(0)
            } else if i.consume_key(M::NONE, K::N) {
                Some(1)
            } else if i.consume_key(M::NONE, K::Enter) || i.consume_key(M::NONE, K::Y) {
                Some(2)
            } else {
                None
            }
        });
        if let Some(p) = pick {
            self.answer_quit(ctx, p);
        }
    }

    /// A save asked for by the question has landed: the window closes, or
    /// stays when the document is still not saved (the failure is under the
    /// bell; a document with no file is told of there too).
    fn quit_after_save(&mut self, ctx: &egui::Context) {
        if self.quit != Some(Quit::Saving) || self.saving > 0 {
            return;
        }
        if self.dirty() {
            self.quit = None;
        } else {
            self.quit = Some(Quit::Go);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    /// The open document's file name (none: empty).
    fn file_name(&self) -> String {
        self.editor
            .as_ref()
            .and_then(engine::Editor::path)
            .and_then(Path::file_name)
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// The question before quitting as the screen reader says it: what it
    /// asks, and the key of each answer.
    fn quit_spoken(&self) -> String {
        let t = &self.table;
        let body = t.trf("quit.body", &[&self.file_name()]);
        t.trf("quit.spoken", &[t.tr("quit.title"), &body, t.tr("quit.save"), t.tr("quit.discard"), t.tr("quit.cancel")])
    }

    /// The question before quitting, over everything; a click is its key.
    fn quit_question(&mut self, ctx: &egui::Context) {
        if self.quit != Some(Quit::Asking) {
            return;
        }
        let name = self.file_name();
        let c = ito_theme::colors();
        let style = ito_confirm::Style {
            panel: c.panel,
            border: c.border,
            accent: Some(c.wait),
            title: c.fg,
            text: c.fg,
            dim: c.dim,
            warn: c.wait,
            button_bg: c.border,
            button_text: c.fg,
        };
        let body = [self.table.trf("quit.body", &[&name])];
        // ito-confirm sits the last button at the right: the way on, Enter's.
        let buttons = [
            format!("{}  Esc", self.table.tr("quit.cancel")),
            format!("{}  N", self.table.tr("quit.discard")),
            format!("{}  Enter", self.table.tr("quit.save")),
        ];
        let q = ito_confirm::Question {
            title: self.table.tr("quit.title"),
            body: &body,
            buttons: &buttons,
            primary: Some(2),
            ..Default::default()
        };
        if let Some(i) = ito_confirm::show(ctx, &q, &style) {
            self.answer_quit(ctx, i);
        }
    }

    /// Typing's clock, in milliseconds since the window opened.
    fn now_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    /// The keys of the frame that the keymap may run: after the fixed ones
    /// (F12, the palette, the scale) took theirs, and not while the palette
    /// is open. A bare character goes to the widget that has the focus, or
    /// to the document while it has the keys.
    fn read_keys(&mut self, ctx: &egui::Context) {
        if self.palette.is_some() || self.quit.is_some() {
            return;
        }
        let typing = ctx.memory(|m| m.focused().is_some()) || self.editing();
        let events = ctx.input(|i| i.events.clone());
        for e in events {
            let k = match e {
                egui::Event::Key { key, pressed: true, modifiers, .. } => ito_keys::from_egui(key, &modifiers),
                egui::Event::Text(t) if !typing => t.chars().next().map(Key::char),
                _ => None,
            };
            if let Some(k) = k {
                self.press(ctx, k);
            }
        }
    }

    /// One key of a chord: run what it completes, wait for more, or drop
    /// what led nowhere (and try the key alone).
    fn press(&mut self, ctx: &egui::Context, k: Key) {
        self.key_pending.push(k);
        let run = match self.keys.resolve(&self.key_pending, self.editing()) {
            Match::Exact(b) => b.run.clone(),
            Match::Pending(_) => return,
            Match::None => {
                let chord = self.key_pending.len() > 1;
                self.key_pending.clear();
                if chord {
                    self.press(ctx, k);
                }
                return;
            }
        };
        self.key_pending.clear();
        for c in run {
            self.run_key(ctx, c);
        }
    }

    /// What a bound key (or the palette) runs. The document's commands run
    /// only while it has the keys.
    fn run_key(&mut self, ctx: &egui::Context, c: keymap::Cmd) {
        let keymap::Cmd::Run(name) = c else { return };
        match name {
            "quit" => self.quit(ctx),
            "settings" => self.open_settings(),
            "help" => self.help.toggle(),
            "file.save" => self.save(),
            "file.open_in_editor" => self.open_in_editor(),
            _ if self.editing() => {
                let (now, page) = (self.now_ms(), self.view.page());
                let Some(ed) = &mut self.editor else { return };
                let done = commands::edit(ed, name, now, page);
                if done.is_some() {
                    self.view.follow = true;
                }
                if done == Some(false) {
                    self.nothing_here(name);
                }
            }
            _ => {}
        }
    }

    /// A command that does not apply at the caret changes nothing on the
    /// screen: the screen reader says so. Only the named ones of the palette
    /// (a structure command, undo): a caret move at the end of the text is
    /// not news.
    fn nothing_here(&mut self, name: &str) {
        let Some(d) = commands::find(name).filter(|d| d.palette) else { return };
        let text = self.table.trf("edit.nothing", &[self.table.tr_desc(d.desc)]);
        self.announce(text);
    }

    /// A job for the file thread; `false` when the thread is gone (told of).
    fn send_file(&mut self, job: FileJob) -> bool {
        if self.files.send(job).is_ok() {
            return true;
        }
        let title = self.table.tr("file.no_thread").to_owned();
        self.outcome(ito_notices::Kind::Err, title, None);
        false
    }

    /// Ctrl+S: what the document is now goes to the file thread.
    fn save(&mut self) {
        let Some(ed) = &self.editor else { return };
        let path = ed.path().map(Path::to_path_buf);
        let name = path.as_deref().map(|p| p.display().to_string()).unwrap_or_default();
        match ed.save_job() {
            Ok(job) => {
                if self.send_file(FileJob::Save(job)) {
                    self.saving += 1;
                }
            }
            Err(engine::NoSave::NoPath) => {
                let title = self.table.tr("file.no_path").to_owned();
                self.outcome(ito_notices::Kind::Warn, title, None);
            }
            Err(engine::NoSave::Locked(_)) => {
                let title = self.table.trf("file.locked", &[&name]);
                self.outcome(ito_notices::Kind::Warn, title, path);
            }
        }
    }

    /// Ctrl+E: `$EDITOR` at the caret's line and column, after a save when
    /// the window has changes (docs/design.md).
    fn open_in_editor(&mut self) {
        let Some(ed) = &self.editor else { return };
        let Some(path) = ed.path().map(Path::to_path_buf) else {
            let title = self.table.tr("file.no_path").to_owned();
            self.outcome(ito_notices::Kind::Warn, title, None);
            return;
        };
        let at = ed.editor_pos();
        let job = ed.is_dirty().then(|| ed.save_job().ok()).flatten();
        if let Some(job) = job {
            if !self.send_file(FileJob::Save(job)) {
                return;
            }
            self.saving += 1;
        }
        let cfg = Box::new(self.prefs.open.clone());
        self.send_file(FileJob::Open { cfg, path, at });
    }

    /// What the file thread finished: a save marks the document clean (at the
    /// version written) or goes under the bell; while the window is behind
    /// another (or minimized), a failed save is also told everywhere
    /// `[notify]` says, and a click on the system's notification comes back
    /// to the file (`clicked`).
    fn take_files(&mut self, ctx: &egui::Context) {
        let behind = ctx.input(|i| !i.focused || i.viewport().minimized == Some(true));
        while let Ok(d) = self.done.try_recv() {
            match d {
                FileDone::Saved { version, path, result } => {
                    self.saving = self.saving.saturating_sub(1);
                    let name = path.display().to_string();
                    match result {
                        Ok(()) => {
                            if let Some(ed) = self.editor.as_mut().filter(|ed| ed.path() == Some(path.as_path())) {
                                ed.saved(version);
                            }
                            let short = path.file_name().map_or(name.clone(), |n| n.to_string_lossy().into_owned());
                            let note = self.table.trf("file.saved", &[&short]);
                            self.announce(note.clone());
                            self.note = Some(note);
                        }
                        Err(e) => {
                            let title = self.table.trf("file.save_failed", &[&name, &e]);
                            let id = self.outcome(ito_notices::Kind::Err, title.clone(), Some(path));
                            if behind {
                                let t = &self.table;
                                notify::tell(&self.prefs.notify, t.tr("notify.save_failed"), &title, id, t.tr("notify.open"));
                            }
                        }
                    }
                }
                FileDone::OpenFailed(e) => {
                    let title = self.table.trf("file.editor_failed", &[&e]);
                    self.outcome(ito_notices::Kind::Err, title, None);
                }
            }
        }
    }

    /// The structure worker: ask for a reading of the text as it is, keep
    /// the newest that came back.
    fn take_tree(&mut self) {
        let Some(ed) = &self.editor else { return };
        self.worker.ask(ed.version(), ed.text());
        if let Some(t) = self.worker.take() {
            if self.tree.as_ref().is_none_or(|old| old.version <= t.version) {
                self.tree = Some(t);
            }
        }
    }

    /// The settings cover the window; the help, if open, goes with it.
    fn open_settings(&mut self) {
        self.help.close();
        self.settings_open = true;
    }

    /// What the window shows, for `<State:name>` and the done file.
    fn report(&self) -> String {
        let screen = if self.settings_open {
            "settings"
        } else if self.help.open {
            "help"
        } else {
            "main"
        };
        let mut out = format!("screen: {screen}\nlang: {}\n", self.lang);
        if self.settings_open {
            out.push_str(&format!("page: {}\n", self.prefs.state.page));
        }
        out.push_str(&format!("palette: {}\nnotices: {}\n", if self.palette.is_some() { "open" } else { "closed" }, self.notices.len()));
        let quit = match self.quit {
            None => "no",
            Some(Quit::Asking) => "asking",
            Some(Quit::Saving) => "saving",
            Some(Quit::Go) => "go",
        };
        out.push_str(&format!("quit: {quit}\n"));
        if let Some(ed) = &self.editor {
            let (line, col) = ed.line_col();
            let file = ed.path().and_then(Path::file_name).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let section = self.section().unwrap_or_default();
            out.push_str(&format!(
                "file: {file}\ndirty: {}\nlocked: {}\ncaret: {}:{}\nsection: {section}\nlines: {}\n",
                ed.is_dirty(),
                ed.locked().is_some(),
                line + 1,
                col + 1,
                ed.line_count(),
            ));
            // The text on one line, as Rust writes a string, cut short.
            let text: String = ed.text().escape_debug().take(400).collect();
            out.push_str(&format!("text: {text}\n"));
        }
        out
    }

    /// The title of the section the caret is in, by the newest reading.
    fn section(&self) -> Option<String> {
        let (ed, tree) = (self.editor.as_ref()?, self.tree.as_ref()?);
        let i = ito_outline::at_byte(&tree.headings, ed.caret())?;
        Some(tree.headings[i].title.clone())
    }

    /// `<State:name>` and the done file: what the window shows, then `tail`.
    fn write_state(&self, path: PathBuf, tail: &str) {
        let text = format!("{}{tail}", self.report());
        if let Err(e) = std::fs::write(&path, text) {
            say(&format!("weft --keys: {}: {e}", path.display()), true);
        }
    }

    /// The steps of the `--keys` script that are not keys: pictures, state
    /// files, the theme and the language (set on this window's copy of
    /// common.toml, which `sync_common` applies; the owner's file is not
    /// written), quit, and the done file.
    fn script(&mut self, ctx: &egui::Context) {
        let Some(script) = &mut self.script else { return };
        if let Some(err) = script.shot(ctx) {
            say(&format!("weft --keys: {err}"), true);
        }
        for d in std::mem::take(&mut self.todo) {
            let Some(script) = &self.script else { return };
            match d {
                Do::State(name) => self.write_state(script.dir.join(format!("{name}.txt")), ""),
                Do::Quit => {
                    // `<Quit>` finishes the run itself, so `on_exit` finds it over and writes nothing.
                    if let Some(done) = script.done.clone() {
                        self.write_state(done, &ito_keyscript::quit_report(script.left()));
                    }
                    // The script's end, not a person's: nothing is asked.
                    self.quit = Some(Quit::Go);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Do::Theme(name) => {
                    self.prefs.common.theme = Some(name.clone());
                    self.prefs.apply_theme(ctx);
                    if let Some(e) = self.prefs.theme_err.take() {
                        say(&format!("weft --keys: <Theme:{name}>: {e}"), true);
                    }
                }
                Do::Lang(code) => {
                    self.prefs.common.language = Some(code.clone());
                    for e in self.sync_lang() {
                        say(&format!("weft --keys: <Lang:{code}>: {e}"), true);
                    }
                }
                // weft defines no readings of its own yet.
                Do::Read { .. } => {}
                Do::Failed { step, why } => {
                    say(&format!("weft --keys: {step}: {why}"), true);
                    if let Some(done) = script.done.clone() {
                        self.write_state(done, &ito_keyscript::failed_report(&step, &why, script.left()));
                    }
                    std::process::exit(ito_keyscript::FAILED_EXIT);
                }
                Do::Done => {
                    if let Some(done) = script.done.clone() {
                        self.write_state(done, "keys: done\n");
                    }
                }
            }
        }
        if let Some(due) = self.script.as_ref().and_then(Runner::due) {
            ctx.request_repaint_after(due);
        }
    }

    /// Registers the faces once the reader thread has the Japanese one: the
    /// monospace face common.toml names (`mono_font`), with the Japanese one
    /// behind both families so names do not draw as boxes.
    fn take_font(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.font else { return };
        let bytes = match rx.try_recv() {
            Ok(b) => b,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(_) => None,
        };
        self.font = None;
        let mut fonts = egui::FontDefinitions::default();
        ito_fonts::register(&mut fonts, self.prefs.common.mono_font_or_default());
        if let Some(bytes) = bytes {
            fonts.font_data.insert("japanese".to_owned(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
            for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts.families.entry(family).or_default().push("japanese".to_owned());
            }
        }
        ctx.set_fonts(fonts);
    }

    /// Ctrl+wheel and Ctrl+= / Ctrl+- / Ctrl+0: the whole window's scale, one
    /// step at a time. It is applied now and written to common.toml off the UI
    /// thread, so the other apps at the same desk follow within 2 seconds.
    fn rescale(&mut self, ctx: &egui::Context) {
        use ito_common::ScaleTo;
        let wheel = ctx.input(|i| if i.modifiers.command { i.smooth_scroll_delta.y } else { 0.0 });
        let to = if wheel > 0.0 {
            Some(ScaleTo::In)
        } else if wheel < 0.0 {
            Some(ScaleTo::Out)
        } else {
            let key = |k| ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, k));
            if key(egui::Key::Equals) || key(egui::Key::Plus) {
                Some(ScaleTo::In)
            } else if key(egui::Key::Minus) {
                Some(ScaleTo::Out)
            } else if key(egui::Key::Num0) {
                Some(ScaleTo::Reset)
            } else {
                None
            }
        };
        if let Some(to) = to {
            self.scale_to(ctx, to);
        }
    }

    fn scale_to(&mut self, ctx: &egui::Context, to: ito_common::ScaleTo) {
        let now = self.prefs.common.scale_or_one();
        let next = ito_common::scale_step(now, to);
        if next == now {
            return;
        }
        self.prefs.set_scale(next, self.table.tr("settings.no_folder"));
        ctx.set_zoom_factor(next);
        ito_statusbar::scale_changed(ctx, next);
        self.note = Some(ito_common::scale_text(next, &self.lang));
    }

    /// common.toml changed (the settings, another app at the same desk, an
    /// editor): the language, the theme and the scale follow; keymap.toml's
    /// change reloads the keys. What a writer could not do, and what the
    /// person's files (common.toml, the themes, config.toml, keymap.toml, the
    /// lang files) got wrong, goes under the bell: each warning once.
    fn sync_common(&mut self, ctx: &egui::Context) {
        self.prefs.pump();
        if let Some(e) = self.prefs.failed.take() {
            self.key_notes.push(e);
        }
        self.take_keymap();
        for e in self.sync_lang() {
            self.key_notes.push(e);
        }
        self.prefs.apply_theme(ctx);
        self.hear_files();
        let scale = self.prefs.common.scale_or_one();
        if ctx.zoom_factor() != scale {
            ctx.set_zoom_factor(scale);
        }
    }

    /// keymap.toml read again: the keys are made again from it (a chord
    /// half typed is dropped), and what it got wrong, or why it could not be
    /// read (the keys stay), goes under the bell if it is news.
    fn take_keymap(&mut self) {
        let now = match self.prefs.keymap_new.take() {
            Some(None) => {
                let (keys, warnings) = keys_of(&self.prefs);
                self.keys = keys;
                self.key_pending.clear();
                warnings
            }
            Some(Some(e)) => vec![e],
            None => return,
        };
        let new = news(&mut self.warned.keymap, &now);
        self.key_notes.extend(new);
    }

    /// What common.toml, the themes folder, the theme common.toml picks and
    /// config.toml warn of now; the warnings not heard before go under the
    /// bell. (The settings show them all, as they are.)
    fn hear_files(&mut self) {
        let (p, w) = (&self.prefs, &mut self.warned);
        let new = [
            news(&mut w.common, &p.common_warn),
            news(&mut w.themes, &p.theme_warn),
            news(&mut w.theme, p.theme_err.as_slice()),
            news(&mut w.config, &p.notify_warn),
        ];
        self.key_notes.extend(new.into_iter().flatten());
    }

    /// The table follows common.toml's language; the lang files' errors come back.
    fn sync_lang(&mut self) -> Vec<String> {
        let lang = i18n::resolve(self.prefs.common.language.as_deref());
        if lang == self.lang {
            return Vec::new();
        }
        let (table, errors) = i18n::table(&lang);
        self.lang = lang;
        self.table = table;
        errors
    }

    /// The bottom bar: on the left the caret's line and column, the section
    /// it is in, what is not saved yet and how the file is written (its line
    /// endings, a BOM); the last thing done (a save, the scale) after them;
    /// the clock from common.toml at the right end.
    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let clock = self.prefs.common.clock_or(None);
        let spec = ito_statusbar::Spec { clock: &clock, lang: &self.lang, opacity: 1.0 };
        let c = ito_theme::colors();
        let t = &self.table;
        let mut items: Vec<(String, egui::Color32)> = Vec::new();
        if let Some(ed) = &self.editor {
            let (line, col) = ed.line_col();
            items.push((t.trf("status.line_col", &[&(line + 1).to_string(), &(col + 1).to_string()]), c.fg));
            if let Some(s) = self.section() {
                items.push((s, c.dim));
            }
            if ed.locked().is_some() {
                items.push((t.tr("status.read_only").to_owned(), c.wait));
            } else if ed.is_dirty() {
                items.push((t.tr("status.unsaved").to_owned(), c.wait));
            }
            if self.saving > 0 {
                items.push((t.tr("status.saving").to_owned(), c.dim));
            }
            let eol = match ed.eol() {
                ito_textbuf::Eol::Lf => Some("status.lf"),
                ito_textbuf::Eol::Crlf => Some("status.crlf"),
                ito_textbuf::Eol::Mixed => Some("status.mixed"),
                ito_textbuf::Eol::None => None,
            };
            if let Some(k) = eol {
                items.push((t.tr(k).to_owned(), c.dim));
            }
            if ed.bom() {
                items.push((t.tr("status.bom").to_owned(), c.dim));
            }
        }
        if let Some(n) = &self.note {
            items.push((n.clone(), c.dim));
        }
        ito_statusbar::show(
            ui,
            &spec,
            |ui| {
                for (s, color) in &items {
                    ito_statusbar::item(ui, s, *color);
                }
            },
            |_| {},
        );
    }

    /// The window's own bar: the logo and the name, the palette box, the bell
    /// and the caption buttons. The bell opens the list of notices.
    fn title_bar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        let unread = ito_notices::unread(&self.notices);
        let bell = self.table.trf("title.bell", &[&unread.to_string()]);
        let spec = ito_titlebar::Spec {
            own: !cfg!(target_os = "macos"),
            left: if cfg!(target_os = "macos") { 76.0 } else { 0.0 },
            maximized,
            unread,
            any_error: ito_notices::any_error(&self.notices),
            centre: true,
            right_min: 0.0,
            words: ito_titlebar::Words {
                search: self.table.tr("title.search"),
                search_key: "Ctrl+Shift+P",
                bell: &bell,
                minimize: self.table.tr("title.minimize"),
                maximize: self.table.tr("title.maximize"),
                restore: self.table.tr("title.restore"),
                close: self.table.tr("title.close"),
            },
        };
        let name = self.table.tr("app.title").to_string();
        let out = ito_titlebar::show(
            ui,
            &spec,
            // The logo, then the name (as the other uchmk apps' title bars).
            |ui, rect| ito_titlebar::brand(ui, rect, &name, icon::pixels),
            |_, _| {},
        );
        if let Some(r) = out.bell_rect {
            self.bell_at = r;
        }
        if out.bell {
            self.bell_open = !self.bell_open;
        }
        if out.search {
            self.palette = Some(ito_palette::View::new());
        }
        if let Some(op) = out.window {
            ito_titlebar::apply(&ctx, op, maximized);
        }
    }

    /// The palette's list: the bug report and the scale, then the commands
    /// in [`commands::ALL`]'s order, with the words the key help uses.
    fn commands(&self) -> Vec<ito_palette::Entry<Command>> {
        let t = &self.table;
        let mut out = vec![
            ito_palette::Entry::new(t.tr("palette.report_bug"), "F12", Command::ReportBug),
            ito_palette::Entry::new(t.tr("palette.reset_scale"), "Ctrl+0", Command::ResetScale),
        ];
        out.extend(commands::palette().map(|d| ito_palette::Entry::new(t.tr_desc(d.desc), self.keys.label(d.name), Command::Run(d.name))));
        out
    }

    /// Ctrl+Shift+P opens the palette; while it is open it takes the keys.
    fn command_palette(&mut self, ctx: &egui::Context) {
        let open = ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::P));
        if open && self.palette.is_none() {
            self.palette = Some(ito_palette::View::new());
        }
        let Some(mut view) = self.palette.take() else { return };
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
        let words = ito_palette::Words { hint: self.table.tr("palette.hint"), none: self.table.tr("palette.none"), header: "", footer: "" };
        match ito_palette::show(ctx, &mut view, &self.commands(), &style, &words) {
            None => self.palette = Some(view),
            Some(ito_palette::Answer::Pick(Command::ReportBug)) => self.report_bug(ctx),
            Some(ito_palette::Answer::Pick(Command::ResetScale)) => self.scale_to(ctx, ito_common::ScaleTo::Reset),
            Some(ito_palette::Answer::Pick(Command::Run(name))) => self.run_key(ctx, keymap::Cmd::Run(name)),
            Some(_) => {}
        }
    }

    /// One line under the bell, about the file `go` (if any); its id.
    fn notice(&mut self, kind: ito_notices::Kind, title: String, go: Option<PathBuf>) -> u64 {
        self.next_notice += 1;
        self.notices.push(ito_notices::Notice {
            id: self.next_notice,
            at: std::time::SystemTime::now(),
            kind,
            title,
            note: None,
            go,
            read: false,
        });
        self.next_notice
    }

    /// How something the person asked for came out: under the bell, and
    /// said to the screen reader (the bell's line takes no focus).
    fn outcome(&mut self, kind: ito_notices::Kind, title: String, go: Option<PathBuf>) -> u64 {
        self.announce(title.clone());
        self.notice(kind, title, go)
    }

    /// `text` for the screen reader, at the end of this frame.
    fn announce(&mut self, text: String) {
        self.said.push(text);
    }

    /// What the files got wrong, and what a writer could not do, go under the
    /// bell. Not said aloud: nobody asked for it just now.
    fn take_notices(&mut self) {
        for title in std::mem::take(&mut self.key_notes) {
            self.notice(ito_notices::Kind::Warn, title, None);
        }
    }

    /// A notice picked (its row under the bell, or a click on its system
    /// notification): it is read, and when it is about the open document,
    /// whatever covers the document goes, so the file is in front.
    fn open_notice(&mut self, id: u64) {
        self.bell_open = false;
        let Some(n) = self.notices.iter_mut().find(|n| n.id == id) else { return };
        n.read = true;
        let open = self.editor.as_ref().and_then(engine::Editor::path);
        if n.go.is_some() && n.go.as_deref() == open {
            self.settings_open = false;
            self.help.close();
            self.palette = None;
        }
    }

    /// The system notifications clicked since the last frame (the click
    /// brought the window forward already).
    fn clicked(&mut self) {
        for id in notify::take_clicks() {
            self.open_notice(id);
        }
    }

    /// The list under the bell, while it is open.
    fn notice_list(&mut self, ctx: &egui::Context) {
        if !self.bell_open {
            return;
        }
        let t = &self.table;
        let title = t.trf("notices.title", &[&ito_notices::unread(&self.notices).to_string()]);
        let words = ito_notices::Words {
            title: &title,
            mark_read: t.tr("notices.mark_read"),
            empty: t.tr("notices.empty"),
            ago: t.tr("notices.ago"),
            kinds: [t.tr("notices.ok"), t.tr("notices.info"), t.tr("notices.warn"), t.tr("notices.err"), t.tr("notices.wait")],
        };
        match ito_notices::show(ctx, self.bell_at, &self.notices, &words) {
            Some(ito_notices::Action::Open(_, id)) => self.open_notice(id),
            Some(ito_notices::Action::ReadAll) => self.notices.iter_mut().for_each(|n| n.read = true),
            Some(ito_notices::Action::Close) => self.bell_open = false,
            None => {}
        }
    }

    /// F12: the browser opens the issue form filled with the version, the OS
    /// and what the app knows.
    fn report_bug(&self, ctx: &egui::Context) {
        let spec = ito_bugreport::Spec { app: "weft", version: env!("CARGO_PKG_VERSION"), repo: REPO, template: "bug_report.yml" };
        let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(std::path::PathBuf::from);
        let context = ito_bugreport::Context { home: home.as_deref(), ..Default::default() };
        let url = ito_bugreport::url(&spec, &[("context", ito_bugreport::context(&context))]);
        ctx.open_url(egui::OpenUrl::new_tab(url));
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.sync_common(&ctx);
        self.clicked();
        self.take_files(&ctx);
        self.quit_after_save(&ctx);
        // The window's close (the bar's ×, Alt+F4) asks first, as Ctrl+Q does.
        if ctx.input(|i| i.viewport().close_requested()) && self.quit != Some(Quit::Go) {
            if self.dirty() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            }
            self.quit(&ctx);
        }
        if self.quit == Some(Quit::Asking) {
            self.quit_keys(&ctx);
        }
        if self.editing() {
            // The document takes the keys without a widget's focus: one left
            // over (the settings, the palette) would type too, and Tab would
            // walk the bar's buttons.
            ctx.memory_mut(|m| {
                if let Some(id) = m.focused() {
                    m.surrender_focus(id);
                }
                m.move_focus(egui::FocusDirection::None);
            });
        }
        // The Keys page is taking a key: every key is that key, F12 too.
        let taking = self.settings_open && self.prefs.keys_edit.busy();
        if !taking && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F12)) {
            self.report_bug(&ctx);
        }
        if !taking {
            self.rescale(&ctx);
        }
        if let Some(t) = &mut self.tracker {
            t.follow(&ctx);
        }
        egui::Panel::top("titlebar")
            .exact_size(ito_titlebar::HEIGHT)
            .frame(egui::Frame::NONE.fill(ito_theme::colors().bg))
            .show(ui, |ui| self.title_bar(ui));
        egui::Panel::bottom("statusbar").exact_size(ito_statusbar::HEIGHT).frame(egui::Frame::NONE).show(ui, |ui| self.status_bar(ui));
        self.take_notices();
        self.notice_list(&ctx);
        if !taking {
            self.command_palette(&ctx);
        }
        self.take_font(&ctx);
        if self.settings_open {
            settings::show(ui, self);
        } else {
            let r = ui.max_rect();
            if self.help.open {
                // Esc and the scrolling keys are the help's before the keymap reads them.
                self.help.keys(&ctx);
            }
            if self.editing() {
                // While the input method composes, its keys (Enter, the arrows) are its own.
                self.view.swallow(&ctx);
            }
            self.read_keys(&ctx);
            // A key may have opened the help or the settings.
            let editing = self.editing();
            let now = self.now_ms();
            if let Some(ed) = &mut self.editor {
                if !editing {
                    ed.seal();
                }
                self.view.show(ui, ed, self.tree.as_ref(), editing, now);
            } else {
                // No file: the ghost stands where the document would be.
                let look = ito_prefs::Look { c: ito_theme::colors(), q: "" };
                let _ = ito_prefs::empty_state(
                    ui,
                    look,
                    ito_prefs::Mark::Info,
                    self.table.tr("empty.title"),
                    self.table.tr("empty.body"),
                    None,
                );
            }
            if self.help.open {
                help::show(ui, self, r);
            }
        }
        self.quit_question(&ctx);
        // What the screen reader hears (and `--keys`' <Announced:> checks).
        for m in std::mem::take(&mut self.said) {
            ito_a11y::announce(ui, &m);
        }
        self.take_tree();
        self.script(&ctx);
        // Last, so the edges of the frameless window win over what is under them.
        if !cfg!(target_os = "macos") {
            ito_titlebar::resize_edges(&ctx);
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        ito_theme::colors().bg.to_normalized_gamma_f32()
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        if let Some(script) = &mut self.script {
            if let Some(d) = script.input(ctx, raw) {
                self.todo.push(d);
            }
        }
    }

    /// The window's last place, and, when the window closed from outside
    /// while a script ran, the done file that says so (else the lane waits
    /// out its timeout).
    fn on_exit(&mut self) {
        // Written now rather than on the tracker's clock.
        if let Some(t) = &mut self.tracker {
            t.close(None);
        }
        let Some(script) = &mut self.script else { return };
        if !script.running() {
            return;
        }
        let left = script.left();
        script.finish();
        let Some(done) = script.done.clone() else { return };
        self.write_state(done, &ito_keyscript::closed_report(left));
    }
}

/// The keys: the person's keymap.toml as `p` last read it (none is fine) over
/// the defaults. What the file got wrong comes back as the lines to show.
fn keys_of(p: &prefs::Prefs) -> (keymap::Keys, Vec<String>) {
    let path = p.base.as_deref().map(|b| prefs::keymap_path(b).display().to_string()).unwrap_or_default();
    keymap::Keys::load(p.keymap.as_deref().map(|t| (path.as_str(), t)))
}

/// `--keys SCRIPT` as its steps; `None` when the arguments are not that.
/// `-x` after `--keys` is the script's text, not an option.
fn keys_arg(args: &[String]) -> Result<Option<Vec<Step>>, String> {
    match args {
        [k, ..] if k != "--keys" => Ok(None),
        [_] => Err("--keys needs a script".to_owned()),
        [_, s] => ito_keyscript::parse(s).map(Some).map_err(|e| format!("--keys: {e}")),
        [_, _, more, ..] => Err(format!("unknown argument `{more}`")),
        [] => Ok(None),
    }
}

/// Reads the Japanese face on a thread of its own (it is tens of megabytes) and repaints when it is in.
fn font_reader(ctx: &egui::Context) -> std::sync::mpsc::Receiver<Option<Vec<u8>>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let _ = tx.send(ito_common::fonts::read_first(ito_common::fonts::JAPANESE).map(|(_, b)| b));
        ctx.request_repaint();
    });
    rx
}

/// Puts a line where whoever typed the command is looking, on standard error
/// when `err`. A release build on Windows has no console of its own;
/// `ito_console` writes to the console it was started from, or to the pipe
/// `weft.com` hands on, and lets go of a write that fails.
fn say(text: &str, err: bool) {
    let line = format!("{text}\n");
    match err {
        true => ito_console::err(&line),
        false => ito_console::out(&line),
    }
}

/// Where the caret starts: `+LINE` or `+LINE:COL`, both from 1, the column
/// in characters (kura's opener passes it as ito-openin's `joined("+", ":")`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Goto {
    line: usize,
    /// 1 when only the line is given.
    col: usize,
}

impl Goto {
    /// `arg` as a position; `None` when it does not start with `+`.
    fn parse(arg: &str) -> Option<Result<Goto, String>> {
        let spec = arg.strip_prefix('+')?;
        let number = |s: &str| s.bytes().all(|b| b.is_ascii_digit()).then(|| s.parse::<usize>().ok()).flatten().filter(|&n| n > 0);
        let (line, col) = match spec.split_once(':') {
            Some((l, c)) => (number(l), number(c)),
            None => (number(spec), Some(1)),
        };
        Some(match (line, col) {
            (Some(line), Some(col)) => Ok(Goto { line, col }),
            _ => Err(format!("bad position `{arg}`: give +LINE or +LINE:COL, both from 1")),
        })
    }

    /// Put the caret of the document just opened there; the first frame
    /// scrolls to it. Without a document, nothing.
    fn apply(self, app: &mut App) {
        if let Some(ed) = &mut app.editor {
            ed.go_to(self.line, self.col);
            app.view.follow = true;
        }
    }
}

/// The file, where its caret starts, and the arguments left over.
type FileArg<'a> = (Option<PathBuf>, Option<Goto>, &'a [String]);

/// `FILE` off the front of the arguments: the first, when it is not an
/// option or `env`, with one `+LINE[:COL]` before or after it. What is left
/// is for [`keys_arg`] and the options.
fn file_arg(args: &[String]) -> Result<FileArg<'_>, String> {
    let mut goto = None;
    let mut args = args;
    if let Some(g) = args.first().and_then(|a| Goto::parse(a)) {
        goto = Some(g?);
        args = &args[1..];
    }
    let file = match args {
        [f, rest @ ..] if !f.starts_with('-') && !f.starts_with('+') && f != "env" => {
            args = rest;
            Some(PathBuf::from(f))
        }
        _ => None,
    };
    if let (None, Some(g)) = (goto, args.first().and_then(|a| Goto::parse(a))) {
        goto = Some(g?);
        args = &args[1..];
    }
    Ok((file, goto, args))
}

fn main() -> eframe::Result {
    let all: Vec<String> = std::env::args().skip(1).collect();
    let done = std::env::var_os("WEFT_KEYS_DONE").filter(|v| !v.is_empty()).map(PathBuf::from);
    let parsed = file_arg(&all).and_then(|(file, goto, args)| keys_arg(args).map(|script| (file, goto, args, script)));
    let (file, goto, args, script) = match parsed {
        Ok(p) => p,
        Err(e) => {
            // A run started detached never sees the message: the done file carries it (ito #193).
            if let Some(done) = &done {
                let _ = std::fs::write(done, ito_keyscript::refused_report(&e));
            }
            say(&format!("weft: {e}\n\n{USAGE}"), true);
            std::process::exit(2);
        }
    };
    if let (Some(_), Some(done)) = (&script, &done) {
        ito_keyscript::install_panic_report(done.clone());
    }
    if let (Some(a), None) = (args.first(), &script) {
        match a.as_str() {
            "--version" | "-V" => say(&ito_bugreport::version_line("weft", env!("CARGO_PKG_VERSION")), false),
            "--help" | "-h" => say(USAGE, false),
            "env" | "--env" => say(envreport::text().trim_end_matches('\n'), false),
            other => {
                say(&format!("weft: unknown argument `{other}`\n\n{USAGE}"), true);
                std::process::exit(2);
            }
        }
        return Ok(());
    }
    // A script plays on a window that keeps no place, and leaves none.
    let saved = if script.is_none() { Saved::load(ito_common::state_dir("weft")) } else { Saved::load(None) };
    let viewport = egui::ViewportBuilder::default()
        .with_title("weft")
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([720.0, 420.0])
        .with_icon(std::sync::Arc::new(egui::IconData { rgba: icon::pixels(ICON_PX), width: ICON_PX, height: ICON_PX }));
    // The bar is the app's own (the Mac keeps its traffic lights over it).
    let viewport = ito_titlebar::frameless(viewport);
    let options = eframe::NativeOptions { viewport: saved.viewport(viewport), ..Default::default() };
    eframe::run_native(
        "weft",
        options,
        Box::new(move |cc| {
            let script = script.map(|steps| {
                let mut r = Runner::new(steps, done);
                r.watch(cc.egui_ctx.clone());
                r
            });
            let mut app = App::new(cc, &saved, script, file);
            if let Some(g) = goto {
                g.apply(&mut app);
            }
            Ok(Box::new(app))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1.8: `--keys` takes the script itself; anything after it is refused.
    #[test]
    fn the_keys_argument_is_a_script_and_anything_after_it_is_refused() {
        let a = |v: &[&str]| keys_arg(&v.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert!(matches!(a(&[]), Ok(None)));
        assert!(matches!(a(&["--version"]), Ok(None)));
        assert_eq!(a(&["--keys", "j<Enter><Shot:main>"]).unwrap().unwrap().len(), 3);
        assert!(matches!(a(&["--keys", "-x"]), Ok(Some(_))));
        assert_eq!(a(&["--keys"]).unwrap_err(), "--keys needs a script");
        assert!(a(&["--keys", "<Nope>"]).unwrap_err().starts_with("--keys:"));
        assert_eq!(a(&["--keys", "j", "--more"]).unwrap_err(), "unknown argument `--more`");
    }

    fn strings(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    /// 9.4: `weft +3:5 FILE` (or `weft FILE +3:5`) opens FILE with the caret
    /// on line 3, column 5: `<State:at>` says `caret: 3:5`.
    #[test]
    fn a_line_and_column_before_or_after_the_file_put_the_caret_there() {
        let dir = std::env::temp_dir().join(format!("weft-goto-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("g.md");
        std::fs::write(&file, "# g\r\n\r\nsome text\r\n").unwrap();
        let name = file.display().to_string();
        for args in [["+3:5", name.as_str()], [name.as_str(), "+3:5"]] {
            let all = strings(&args);
            let (path, goto, rest) = file_arg(&all).unwrap();
            assert_eq!(goto, Some(Goto { line: 3, col: 5 }));
            assert!(rest.is_empty());
            let mut app = App::for_tests();
            app.open(&path.unwrap());
            goto.unwrap().apply(&mut app);
            assert!(app.report().contains("caret: 3:5\n"), "{}", app.report());
            assert!(app.view.follow, "the first frame scrolls to it");
        }
        let all = strings(&["+40", &name]);
        let (path, goto, _) = file_arg(&all).unwrap();
        let mut app = App::for_tests();
        app.open(&path.unwrap());
        goto.unwrap().apply(&mut app);
        assert!(app.report().contains("caret: 4:1\n"), "past the end is the last line: {}", app.report());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 9.6: A bad `+LINE[:COL]` (`+`, `+0`, `+x`, `+3:`) is refused as any
    /// bad argument is; a second one is an unknown argument.
    #[test]
    fn the_position_argument_takes_a_line_and_a_column_from_one() {
        let a = |v: &[&str]| file_arg(&strings(v)).map(|(f, g, rest)| (f, g, rest.to_vec()));
        let at = |line, col| Some(Goto { line, col });
        assert_eq!(a(&["+12", "a.md"]).unwrap(), (Some("a.md".into()), at(12, 1), vec![]));
        assert_eq!(a(&["a.md", "+12:7"]).unwrap(), (Some("a.md".into()), at(12, 7), vec![]));
        assert_eq!(a(&["+2", "a.md", "--keys", "j"]).unwrap(), (Some("a.md".into()), at(2, 1), strings(&["--keys", "j"])));
        assert_eq!(a(&["a.md", "+2", "--keys", "j"]).unwrap().2, strings(&["--keys", "j"]));
        assert_eq!(a(&["+2"]).unwrap(), (None, at(2, 1), vec![]));
        assert_eq!(a(&["a.md"]).unwrap(), (Some("a.md".into()), None, vec![]));
        assert_eq!(a(&["./+a.md"]).unwrap(), (Some("./+a.md".into()), None, vec![]));
        assert_eq!(a(&["+1", "a.md", "+2"]).unwrap().2, strings(&["+2"]), "one position: the second is left over");
        for bad in ["+", "+0", "+x", "+3:", "+:3", "+3:0", "+3:x", "++3", "+-3", "+3:4:5", "+99999999999999999999999"] {
            assert_eq!(a(&[bad, "a.md"]).unwrap_err(), format!("bad position `{bad}`: give +LINE or +LINE:COL, both from 1"));
            assert!(a(&["a.md", bad]).is_err(), "{bad} after the file");
        }
    }

    /// 4.2: The palette and the keymap name the same things: every command
    /// the palette lists is in it with its key and the key help's words.
    #[test]
    fn every_palette_command_is_in_the_palette_with_its_key() {
        let app = App::for_tests();
        let entries = app.commands();
        assert_eq!(entries.len(), 2 + commands::palette().count());
        for d in commands::palette() {
            let key = app.keys.label(d.name);
            let title = app.table.tr_desc(d.desc);
            assert!(entries.iter().any(|e| e.key == key && e.title == title), "{} ({key}) is not in the palette", d.name);
        }
        let quit = entries.iter().find(|e| matches!(e.pick, Command::Run("quit"))).unwrap();
        assert_eq!(quit.key, "Ctrl+Q");
    }

    /// The status bar names the section the caret is in, at the very end of
    /// the text too.
    #[test]
    fn the_section_is_found_with_the_caret_at_the_end() {
        let mut app = App::for_tests();
        let mut ed = engine::Editor::new("# One\n\na\n\n## Two\n\nb");
        ed.set_caret(ed.text().len(), false);
        app.tree = Some(structure::read(ed.version(), ed.text()));
        app.editor = Some(ed);
        assert_eq!(app.section().as_deref(), Some("Two"));
    }

    /// 8.6: Quitting with changes not saved asks first; Esc stays, N goes
    /// without writing, a save that cannot happen leaves the window open, and
    /// one that lands closes it.
    #[test]
    fn quitting_with_changes_asks_and_closes_once_the_save_lands() {
        let ctx = egui::Context::default();
        let mut app = App::for_tests();
        app.quit(&ctx);
        assert_eq!(app.quit, Some(Quit::Go), "nothing open: nothing to lose");

        let mut app = App::for_tests();
        let mut ed = engine::Editor::new("a");
        ed.insert("b", 0);
        app.editor = Some(ed);
        app.quit(&ctx);
        app.answer_quit(&ctx, 1);
        assert_eq!(app.quit, Some(Quit::Go), "N quits as it is");
        assert_eq!(app.saving, 0, "and writes nothing");

        let mut app = App::for_tests();
        let mut ed = engine::Editor::new("a");
        ed.insert("b", 0);
        app.editor = Some(ed);
        app.quit(&ctx);
        assert_eq!(app.quit, Some(Quit::Asking));
        assert!(!app.editing(), "the question has the keys");
        app.answer_quit(&ctx, 0);
        assert_eq!(app.quit, None, "Esc stays");
        assert!(app.editing());
        // No file to save to: told under the bell, and the window stays.
        app.quit(&ctx);
        app.answer_quit(&ctx, 2);
        app.quit_after_save(&ctx);
        assert_eq!(app.quit, None);
        assert_eq!(app.notices.len(), 1);

        let dir = std::env::temp_dir().join(format!("weft-quit-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("q.md");
        std::fs::write(&file, "a").unwrap();
        let mut app = App::for_tests();
        app.open(&file);
        app.editor.as_mut().unwrap().insert("b", 0);
        app.quit(&ctx);
        app.answer_quit(&ctx, 2);
        assert_eq!(app.quit, Some(Quit::Saving));
        let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while app.saving > 0 && std::time::Instant::now() < until {
            std::thread::sleep(std::time::Duration::from_millis(10));
            app.take_files(&ctx);
        }
        app.quit_after_save(&ctx);
        assert_eq!(app.quit, Some(Quit::Go), "saved, so the window closes");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "ba");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 5.4: What the person's files got wrong goes under the bell once: the
    /// theme picked every frame, or a file read again, adds nothing; a
    /// warning that went away and came back is told again.
    #[test]
    fn a_file_warning_goes_under_the_bell_once() {
        let mut app = App::for_tests();
        let frame = |app: &mut App| {
            app.take_keymap();
            app.hear_files();
            app.take_notices();
        };
        app.prefs.theme_err = Some("theme `nope` is not there".to_owned());
        app.prefs.common_warn = vec!["common.toml: scale: not a number".to_owned()];
        app.prefs.notify_warn = vec!["config.toml: [notify] system: `done` is not an event".to_owned()];
        for _ in 0..3 {
            frame(&mut app);
        }
        assert_eq!(app.notices.len(), 3, "{:?}", app.notices);
        assert!(app.notices.iter().all(|n| n.kind == ito_notices::Kind::Warn && n.go.is_none()));
        assert!(app.said.is_empty(), "nobody asked: nothing is said aloud");

        // The theme is found, then lost again.
        app.prefs.theme_err = None;
        frame(&mut app);
        app.prefs.theme_err = Some("theme `nope` is not there".to_owned());
        frame(&mut app);
        assert_eq!(app.notices.len(), 4);

        // keymap.toml broken, read twice; then unreadable.
        app.prefs.keymap = Some("[edit\n".to_owned());
        app.prefs.keymap_new = Some(None);
        frame(&mut app);
        let broken = app.notices.len();
        assert!(broken > 4, "a broken keymap.toml is told");
        app.prefs.keymap_new = Some(None);
        frame(&mut app);
        assert_eq!(app.notices.len(), broken, "read again, it says nothing new");
        app.prefs.keymap_new = Some(Some("keymap.toml: Access is denied.".to_owned()));
        frame(&mut app);
        assert_eq!(app.notices.len(), broken + 1);
    }

    /// 5.5: A notice about the open file, picked under the bell (or by a
    /// click on its system notification), puts the document in front: the
    /// settings, the help and the palette go. One about another file is
    /// only read.
    #[test]
    fn a_notice_about_the_open_file_puts_it_in_front() {
        let dir = std::env::temp_dir().join(format!("weft-notice-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("n.md");
        std::fs::write(&file, "a").unwrap();
        let mut app = App::for_tests();
        app.open(&file);
        let other = app.notice(ito_notices::Kind::Err, "b.md".to_owned(), Some(dir.join("b.md")));
        let here = app.outcome(ito_notices::Kind::Err, "n.md".to_owned(), Some(file.clone()));
        assert_ne!(other, here);
        let read = |app: &App, id| app.notices.iter().find(|n| n.id == id).unwrap().read;

        app.settings_open = true;
        app.bell_open = true;
        app.open_notice(other);
        assert!(read(&app, other) && !app.bell_open);
        assert!(app.settings_open, "another file's notice leaves the screen as it is");

        app.palette = Some(ito_palette::View::new());
        app.open_notice(here);
        assert!(read(&app, here));
        assert!(!app.settings_open && app.palette.is_none() && !app.help.open);
        assert!(app.editing(), "the document has the keys");
        // An id that is no notice (a click on an old run's notification).
        app.open_notice(99);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 7.3: What a screen reader hears: a save and one that cannot be, a
    /// command that does nothing at the caret, and the question before
    /// quitting (once, with its keys); a caret move that goes nowhere is
    /// not news.
    #[test]
    fn the_screen_reader_hears_saves_nothing_to_do_and_the_quit_question() {
        let ctx = egui::Context::default();
        let mut app = App::for_tests();
        app.editor = Some(engine::Editor::new("a"));
        app.run_key(&ctx, keymap::Cmd::Run("edit.undo"));
        assert_eq!(app.said, ["Undo: nothing to do here"]);
        app.run_key(&ctx, keymap::Cmd::Run("cursor.left"));
        assert_eq!(app.said.len(), 1, "a move at the start is not news");
        app.run_key(&ctx, keymap::Cmd::Run("file.save"));
        assert_eq!(app.said.len(), 2);
        assert!(app.said[1].starts_with("This document has no file yet"), "{:?}", app.said);
        assert_eq!(app.notices.len(), 1, "and it is under the bell");

        app.said.clear();
        app.editor.as_mut().unwrap().insert("b", 0);
        app.quit(&ctx);
        app.quit(&ctx);
        assert_eq!(app.said.len(), 1, "asked twice (the window's close), said once");
        let q = &app.said[0];
        assert!(q.starts_with("Unsaved changes. ") && q.contains("Enter: Save and quit") && q.ends_with("Esc: Cancel"), "{q}");

        let dir = std::env::temp_dir().join(format!("weft-said-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("s.md");
        std::fs::write(&file, "a").unwrap();
        let mut app = App::for_tests();
        app.open(&file);
        app.editor.as_mut().unwrap().insert("b", 0);
        app.save();
        let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while app.saving > 0 && std::time::Instant::now() < until {
            std::thread::sleep(std::time::Duration::from_millis(10));
            app.take_files(&ctx);
        }
        assert_eq!(app.said, ["Saved s.md"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
