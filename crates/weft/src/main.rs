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

mod big;
mod changes;
mod commands;
mod count;
mod door;
mod engine;
mod envreport;
mod external;
mod find;
mod fold;
mod foldkeep;
mod help;
mod i18n;
mod icon;
mod image;
mod keymap;
mod lint;
mod notify;
mod prefs;
mod preview;
mod recovery;
mod settings;
mod structure;
mod tabs;
mod view;
mod wrap;

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender};

use ito_common::window::{Saved, Tracker};
use ito_keys::Key;
use ito_keyscript::{Do, Runner, Step};

/// The window's icon, in pixels square.
/// Above this many bytes the text goes to the structure and gutter workers at most every `BIG_PAUSE`.
const BIG_TEXT: usize = 512 * 1024;
const BIG_PAUSE: std::time::Duration = std::time::Duration::from_millis(150);
const ICON_PX: u32 = 128;

const REPO: &str = "https://github.com/uchmk/weft";

const USAGE: &str = "\
weft — Markdown 専用エディター。キーだけで見出し・リスト・表などの構造を組み替え、日本語 IME に強く、フォルダー（vault）も扱う。kura の .md の opener

usage: weft [+LINE[:COL]] [FILE [FILE...]] [--keys SCRIPT]
       weft --version | --help | env

  FILE           the Markdown file to edit (one that is not there yet is
                 made when it is saved); more FILEs open as
                 tabs behind it
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
    /// Read what the file holds now (someone else may have changed it).
    Check {
        path: PathBuf,
    },
    /// `$EDITOR` at the caret. After a save that failed, it is not run: the
    /// editor would open what is on the disk, not what the window shows.
    Open {
        // Boxed: far larger than a save job on some targets.
        cfg: Box<ito_openin::Config>,
        path: PathBuf,
        at: ito_openin::Pos,
    },
    /// The clipboard's picture, stored next to the note (src/image.rs).
    PasteImage {
        path: PathBuf,
        stamp: String,
    },
}

/// What came back from the file thread.
enum FileDone {
    /// `bytes` is what was written (BOM and all): what the file holds now.
    Saved {
        version: u64,
        path: PathBuf,
        result: Result<(), String>,
        bytes: String,
    },
    /// What the file holds, `None` when it is gone or not UTF-8.
    Disk {
        path: PathBuf,
        raw: Option<String>,
    },
    OpenFailed(String),
    /// The picture stored: the link's path, `None` when the clipboard had none.
    Image {
        path: PathBuf,
        result: Result<Option<String>, String>,
    },
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
                    FileDone::Saved { version: job.version, path: job.path, result, bytes: job.bytes }
                }
                FileJob::Check { path } => {
                    let raw = std::fs::read(&path).ok().and_then(|b| String::from_utf8(b).ok());
                    FileDone::Disk { path, raw }
                }
                FileJob::PasteImage { path, stamp } => {
                    let result = image::paste_job(&path, &stamp);
                    FileDone::Image { path, result }
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

/// One document and everything that belongs to it alone: the editor, how it
/// is shown, its newest structure reading and the thread that makes it, the
/// selection's count, and how many of its saves are on the file thread.
struct Doc {
    /// The editor, when a file is open.
    editor: Option<engine::Editor>,
    /// A file of 8 MiB or more: the lines are read from the disk, and `editor` is empty and locked.
    big: Option<big::Big>,
    view: view::View,
    /// The newest reading of the structure, and the thread that reads it.
    tree: Option<structure::Tree>,
    worker: structure::Worker,
    /// What changed since the file was read or saved, for the gutter.
    changes: changes::Worker,
    /// What the file held when the person kept the window's text over it (not asked again).
    declined: Option<String>,
    /// The selection's character count for the status bar, kept between frames.
    selected: count::Selected,
    saving: usize,
    /// The find bar's search of this text: its thread and newest matches.
    finder: find::DocFinder,
    /// The rendered preview of this text: its thread, layout and scroll.
    preview: preview::DocPreview,
    /// When the text last went to the structure and gutter workers (a big text is not sent on every key).
    sent_at: Option<std::time::Instant>,
}

impl Doc {
    /// A document with nothing open; `wake` is asked for a repaint when a
    /// worker has a result.
    fn new(wake: impl Fn() + Send + Sync + 'static) -> Self {
        let wake = std::sync::Arc::new(wake);
        let for_find = wake.clone();
        let for_preview = wake.clone();
        let for_changes = wake.clone();
        Self {
            editor: None,
            big: None,
            view: view::View::default(),
            tree: None,
            worker: structure::Worker::new(move || wake()),
            changes: changes::Worker::new(move || for_changes()),
            declined: None,
            selected: count::Selected::default(),
            saving: 0,
            finder: find::DocFinder::new(move || for_find()),
            preview: preview::DocPreview::new(move || for_preview()),
            sent_at: None,
        }
    }
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
    /// The document in front (`weft FILE`) and what goes with it.
    doc: Doc,
    /// The find and replace bar (src/find.rs), shared by the tabs, and whether
    /// a widget of it has the focus (then the document does not type).
    find: ito_find::FindBar,
    find_focus: bool,
    /// The rendered preview beside the source is wanted (kept in preview.toml).
    preview_open: bool,
    preview_writer: ito_common::kept::Writer,
    /// Long lines fold at the pane's width (kept in `wrap.toml`).
    wrap: bool,
    wrap_writer: ito_common::kept::Writer,
    /// The window, for the workers of the documents opened later.
    ctx: egui::Context,
    /// The open files as tabs (src/tabs.rs); `doc` is the front one's
    /// document, `parked` the others'.
    tabs: ito_tabs::Tabs<PathBuf>,
    parked: Vec<(PathBuf, Doc)>,
    /// Where the tabs are kept between runs (nothing is written for `--keys`).
    tab_writer: ito_common::kept::Writer,
    /// The tab whose closing waits on the question (it has changes not
    /// saved), and the tabs a save will close once it lands.
    closing: Option<PathBuf>,
    close_when_saved: Vec<PathBuf>,
    /// The folders of the open files, watched for changes made by others
    /// (src/external.rs); the files waiting to be read after their events
    /// settle, and the ones whose question is up.
    dirwatch: ito_common::dirwatch::DirWatcher,
    watch_wait: Vec<(PathBuf, std::time::Instant)>,
    asked: Vec<external::Asked>,
    /// Unsaved text copied for a crash (src/recovery.rs): where, what was
    /// last copied, when, and what a crashed run left (the question's list).
    recovery_writer: ito_common::kept::Writer,
    /// The folds kept for the files last folded, and the writer of `folds.toml`.
    fold_store: Vec<foldkeep::Entry>,
    fold_writer: ito_common::kept::Writer,
    recovery_marks: Vec<(PathBuf, u64)>,
    recovery_at: std::time::Instant,
    recover: Vec<recovery::Entry>,
    /// The file thread's way in and out.
    files: Sender<FileJob>,
    done: Receiver<FileDone>,
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
    /// Whether the input method is on and its input mode, as the status bar
    /// last read it; `None` where the platform cannot tell.
    ime: Option<ito_ime::Status>,
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
    /// The keyboard is a Mac's: Option types characters (`å`), so an Alt
    /// chord the keymap does not take types its character.
    mac: bool,
    /// The door a second `weft FILE` knocks on (`None`: a window of its own).
    door: Option<ito_ipc::single::Owner<ito_ipc::single::OpenRequest>>,
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

/// The look of the questions before quitting or closing a tab.
fn confirm_style() -> ito_confirm::Style {
    let c = ito_theme::colors();
    ito_confirm::Style {
        panel: c.panel,
        border: c.border,
        accent: Some(c.wait),
        title: c.fg,
        text: c.fg,
        dim: c.dim,
        warn: c.wait,
        button_bg: c.border,
        button_text: c.fg,
    }
}

/// What the palette can do: the two fixed keys' commands, and every command
/// of [`commands::palette`] by name (as a key runs it).
#[derive(Clone, Copy)]
enum Command {
    ReportBug,
    ResetScale,
    Run(&'static str),
}

/// What one side of the status bar says: each item's words and colour.
type StatusItems = Vec<(String, egui::Color32)>;

/// The lang key the status bar names an input mode of a switched-on input
/// method with.
fn ime_mode_key(mode: ito_ime::Mode) -> &'static str {
    match mode {
        ito_ime::Mode::Hiragana => "status.ime_hiragana",
        ito_ime::Mode::FullKatakana => "status.ime_full_katakana",
        ito_ime::Mode::HalfKatakana => "status.ime_half_katakana",
        ito_ime::Mode::FullAlnum => "status.ime_full_alnum",
        ito_ime::Mode::HalfAlnum => "status.ime_half_alnum",
        ito_ime::Mode::Other => "status.ime_on",
    }
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
        // The monospace face common.toml names (`mono_font`), with the
        // Japanese and emoji faces behind both families so names do not draw
        // as boxes: read and set on a thread of their own (the Japanese face
        // is tens of megabytes), so the window does not wait for them.
        ito_fonts::load_in_background(cc.egui_ctx.clone(), prefs.common.mono_font_or_default());
        let wake = cc.egui_ctx.clone();
        let (files, done) = file_thread(cc.egui_ctx.clone());
        // A click on a system notification weft showed brings this window forward.
        notify::set_window(&cc.egui_ctx);
        let ctx = wake.clone();
        let tab_settings = prefs::tabs_now(ito_common::base_dir().as_deref());
        let live = script.is_none();
        let tab_path = ito_tabs::state_path("weft");
        let mut app = Self {
            table,
            prefs,
            settings_open: false,
            help: help::Help::default(),
            tracker: saved.tracker(),
            lang,
            note: None,
            doc: Doc::new(move || wake.request_repaint()),
            find: ito_find::FindBar::default(),
            find_focus: false,
            preview_open: live && preview::read_open(),
            preview_writer: ito_common::kept::Writer::new(
                preview::state_path().unwrap_or_default(),
                live && preview::state_path().is_some(),
            ),
            wrap: live && wrap::read(),
            wrap_writer: ito_common::kept::Writer::new(wrap::state_path().unwrap_or_default(), live && wrap::state_path().is_some()),
            ctx,
            tabs: ito_tabs::Tabs::new(tab_settings.limit),
            parked: Vec::new(),
            tab_writer: ito_common::kept::Writer::new(tab_path.clone().unwrap_or_default(), live && tab_path.is_some()),
            closing: None,
            close_when_saved: Vec::new(),
            dirwatch: {
                let ctx = cc.egui_ctx.clone();
                ito_common::dirwatch::DirWatcher::new(move || ctx.request_repaint())
            },
            watch_wait: Vec::new(),
            asked: Vec::new(),
            recovery_writer: ito_common::kept::Writer::new(
                recovery::state_path().unwrap_or_default(),
                live && recovery::state_path().is_some(),
            ),
            fold_store: if live || std::env::var_os("WEFT_STATE_HOME").is_some() { foldkeep::read() } else { Vec::new() },
            fold_writer: ito_common::kept::Writer::new(
                foldkeep::state_path().unwrap_or_default(),
                live && foldkeep::state_path().is_some(),
            ),
            recovery_marks: Vec::new(),
            recovery_at: std::time::Instant::now() - std::time::Duration::from_secs(60),
            recover: if live || std::env::var_os("WEFT_STATE_HOME").is_some() { recovery::read() } else { Vec::new() },
            files,
            done,
            started: std::time::Instant::now(),
            notices: Vec::new(),
            next_notice: 0,
            bell_open: false,
            bell_at: egui::Rect::NOTHING,
            palette: None,
            ime: None,
            script,
            todo: Vec::new(),
            keys: keymap::Keys::load(None).0,
            key_pending: Vec::new(),
            key_notes,
            warned: Warned::default(),
            said: Vec::new(),
            quit: None,
            mac: cfg!(target_os = "macos"),
            door: None,
        };
        // What the person's files got wrong goes under the bell, once.
        app.take_keymap();
        app.hear_files();
        for w in &app.key_notes {
            eprintln!("weft: {w}");
        }
        // The tabs of the last run come back (a script starts with none), then the file given.
        if live && tab_settings.reopen == ito_tabs::Reopen::LastTabs {
            if let Some(p) = &tab_path {
                app.restore_tabs(p);
            }
        }
        if let Some(path) = file {
            app.open(&path);
        }
        app
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
            doc: Doc::new(|| {}),
            find: ito_find::FindBar::default(),
            find_focus: false,
            preview_open: false,
            preview_writer: ito_common::kept::Writer::new(PathBuf::new(), false),
            wrap: false,
            wrap_writer: ito_common::kept::Writer::new(PathBuf::new(), false),
            ctx: egui::Context::default(),
            tabs: ito_tabs::Tabs::new(ito_tabs::DEFAULT_LIMIT),
            parked: Vec::new(),
            tab_writer: ito_common::kept::Writer::new(PathBuf::new(), false),
            closing: None,
            close_when_saved: Vec::new(),
            dirwatch: ito_common::dirwatch::DirWatcher::new(|| {}),
            watch_wait: Vec::new(),
            asked: Vec::new(),
            fold_store: Vec::new(),
            fold_writer: ito_common::kept::Writer::new(PathBuf::new(), false),
            recovery_writer: ito_common::kept::Writer::new(PathBuf::new(), false),
            recovery_marks: Vec::new(),
            recovery_at: std::time::Instant::now() - std::time::Duration::from_secs(60),
            recover: Vec::new(),
            files,
            done,
            started: std::time::Instant::now(),
            notices: Vec::new(),
            next_notice: 0,
            bell_open: false,
            bell_at: egui::Rect::NOTHING,
            palette: None,
            ime: None,
            script: None,
            todo: Vec::new(),
            keys,
            key_pending: Vec::new(),
            key_notes: Vec::new(),
            warned: Warned::default(),
            said: Vec::new(),
            quit: None,
            mac: cfg!(target_os = "macos"),
            door: None,
        }
    }

    /// The document has the keys: one is open and nothing covers it (the
    /// settings, the help, the palette, the list under the bell, the
    /// question before quitting).
    fn editing(&self) -> bool {
        self.uncovered() && !self.find_focus
    }

    /// As `editing`, but also while the find bar has the focus: nothing covers
    /// the document, so the commands that need only that (tabs, find) run.
    fn uncovered(&self) -> bool {
        self.doc.editor.is_some()
            && !self.settings_open
            && !self.help.open
            && self.palette.is_none()
            && !self.bell_open
            && self.quit.is_none()
            && self.closing.is_none()
    }

    /// Some open document has changes not yet saved.
    fn dirty(&self) -> bool {
        self.any_dirty()
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
            self.closing = None;
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
                self.save_all();
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
        if self.quit != Some(Quit::Saving) || self.saving() > 0 {
            return;
        }
        if self.dirty() {
            self.quit = None;
        } else {
            self.quit = Some(Quit::Go);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    /// The question before quitting as the screen reader says it: what it
    /// asks, and the key of each answer.
    fn quit_spoken(&self) -> String {
        let t = &self.table;
        let body = self.quit_body();
        t.trf("quit.spoken", &[t.tr("quit.title"), &body, t.tr("quit.save"), t.tr("quit.discard"), t.tr("quit.cancel")])
    }

    /// The question before quitting, over everything; a click is its key.
    fn quit_question(&mut self, ctx: &egui::Context) {
        if self.quit != Some(Quit::Asking) {
            return;
        }
        let style = confirm_style();
        let body = [self.quit_body()];
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

    /// The frame's keys and typing, in the order they came (after the fixed
    /// keys, F12, the palette and the scale, took theirs), and nothing while
    /// the palette or the question before quitting is up.
    ///
    /// [`ito_keys::read_for`] gives the keys, the text and the input method's
    /// commits in one list: a key goes to the keymap, text and commits to the
    /// document while it has the keys (the view draws the composition and
    /// places the candidate window, but inserts nothing). Windows sends a
    /// chord and the character it would type (Alt+X and `x`): the character
    /// is dropped when the keymap takes the key, and after any Alt chord but
    /// on a Mac, where Option types characters. Copy, cut and paste go to the
    /// document, never to the keymap. Text with no document to take it is a
    /// bare key for the keymap, unless a widget (a field of the settings) has
    /// the focus and reads it.
    fn read_keys(&mut self, ctx: &egui::Context) {
        if self.palette.is_some() || self.quit.is_some() || self.closing.is_some() || self.external_open() || self.recover_open() {
            return;
        }
        let focused = ctx.memory(|m| m.focused().is_some());
        let (events, now) = ctx.input(|i| (i.events.clone(), i.modifiers));
        // Copy, cut and paste, in order, as the keys `read_for` makes of them.
        // egui sends those chords as these events and never as keys, so a key
        // that is the next of them is that event.
        let clips: Vec<(Key, usize)> = (0..events.len())
            .filter(|&i| matches!(events[i], egui::Event::Cut | egui::Event::Copy | egui::Event::Paste(_)))
            .filter_map(|i| ito_keys::from_clipboard(&events, i, &now).map(|k| (k, i)))
            .collect();
        // Whether the keymap takes each key: pressed in turn on a copy of the
        // chord so far, as `press` will press them.
        let editing = self.editing();
        let mut pending = self.key_pending.clone();
        let mut next_clip = 0;
        let inputs = ito_keys::read_for(
            &events,
            &now,
            |k| {
                if clips.get(next_clip).is_some_and(|(c, _)| c == k) {
                    next_clip += 1;
                    return false;
                }
                self.keys.press(&mut pending, *k, editing).took()
            },
            self.mac,
        );
        let mut clips = clips.into_iter().peekable();
        for input in inputs {
            match input {
                ito_keys::Input::Key(k) => match clips.next_if(|(c, _)| *c == k) {
                    Some((_, i)) => self.clipboard(ctx, &events[i]),
                    None => self.press(ctx, k),
                },
                ito_keys::Input::Text(t) if self.editing() => self.type_text(&t),
                ito_keys::Input::Text(t) if !focused => {
                    if let Some(c) = t.chars().next() {
                        self.press(ctx, Key::char(c));
                    }
                }
                ito_keys::Input::Text(_) => {}
                ito_keys::Input::Ime(egui::ImeEvent::Commit(t)) if self.editing() => self.commit(&t),
                ito_keys::Input::Ime(_) => {}
            }
        }
    }

    /// One key of a chord: run what it completes, wait for more, or drop
    /// what led nowhere (and try the key alone).
    fn press(&mut self, ctx: &egui::Context, k: Key) {
        let editing = self.editing();
        if let keymap::Pressed::Run(run) = self.keys.press(&mut self.key_pending, k, editing) {
            for c in run {
                self.run_key(ctx, c);
            }
        }
    }

    /// What the input method committed, into the document as an undo step of
    /// its own.
    fn commit(&mut self, t: &str) {
        let now = self.now_ms();
        let Some(ed) = &mut self.doc.editor else { return };
        if ed.commit(t, now) {
            self.doc.view.follow = true;
        }
    }

    /// Typed text into the document. Control characters are not typed: Enter
    /// and Tab are keys.
    fn type_text(&mut self, t: &str) {
        let now = self.now_ms();
        let Some(ed) = &mut self.doc.editor else { return };
        let t: String = t.chars().filter(|c| !c.is_control()).collect();
        if ed.insert(&t, now) {
            self.doc.view.follow = true;
        }
    }

    /// Copy, cut or paste (`e`), while the document has the keys. A widget
    /// with the focus reads its own.
    fn clipboard(&mut self, ctx: &egui::Context, e: &egui::Event) {
        if !self.editing() {
            return;
        }
        let now = self.now_ms();
        let Some(ed) = &mut self.doc.editor else { return };
        let changed = match e {
            egui::Event::Copy => {
                if let Some(s) = ed.selected_text() {
                    ctx.copy_text(s);
                }
                false
            }
            egui::Event::Cut => match ed.cut(now) {
                Some(s) => {
                    ctx.copy_text(s);
                    true
                }
                None => false,
            },
            egui::Event::Paste(t) => ed.paste(t, now),
            _ => false,
        };
        self.doc.view.follow |= changed;
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
            "edit.paste_image" => self.paste_image(),
            "view.toggle_wrap" => self.wrap_toggle(ctx),
            "view.toggle_preview" => self.preview_toggle(ctx),
            _ if ito_tabs::command(name).is_some() => {
                if let (true, Some(op)) = (self.uncovered(), ito_tabs::command(name)) {
                    self.tab_op(op);
                }
            }
            "heading.fold" | "heading.fold_all" | "heading.unfold_all" => self.fold_command(name),
            "view.focus" => self.focus_toggle(),
            "lint.next" | "lint.prev" | "lint.fix_all" => self.lint_command(name),
            ito_find::cmd::OPEN
            | ito_find::cmd::REPLACE_OPEN
            | ito_find::cmd::NEXT
            | ito_find::cmd::PREV
            | ito_find::cmd::REPLACE_ONE
            | ito_find::cmd::REPLACE_ALL => {
                if self.uncovered() {
                    self.find_command(ctx, name);
                }
            }
            _ if self.editing() => {
                let (now, page) = (self.now_ms(), self.doc.view.page());
                let (tab, tab_char) = (self.prefs.tab_width, self.prefs.tab_char);
                let Some(ed) = &mut self.doc.editor else { return };
                // The settings' tab width and tab character, for Tab outside
                // tables and lists.
                ed.set_tab_width(tab);
                ed.set_tab_char(tab_char);
                let done = commands::edit(ed, name, now, page);
                let unflanked = ed.take_unflanked();
                if done.is_some() {
                    self.doc.view.follow = true;
                }
                if done == Some(false) {
                    self.nothing_here(name);
                }
                if unflanked {
                    self.unflanked();
                }
            }
            _ => {}
        }
    }

    /// A format command put its marks in next to punctuation, where Markdown
    /// may not read them as emphasis: one line on the status bar, said to the
    /// screen reader. Not under the bell, and nothing to answer.
    fn unflanked(&mut self) {
        let text = self.table.tr("edit.unflanked").to_owned();
        self.announce(text.clone());
        self.note = Some(text);
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
        if let Some(p) = self.doc.path() {
            self.save_at(&p);
        } else if self.doc.editor.is_some() {
            let title = self.table.tr("file.no_path").to_owned();
            self.outcome(ito_notices::Kind::Warn, title, None);
        }
    }

    /// Ctrl+E: `$EDITOR` at the caret's line and column, after a save when
    /// the window has changes (docs/design.md).
    fn open_in_editor(&mut self) {
        let Some(ed) = &self.doc.editor else { return };
        let Some(path) = ed.path().map(Path::to_path_buf) else {
            let title = self.table.tr("file.no_path").to_owned();
            self.outcome(ito_notices::Kind::Warn, title, None);
            return;
        };
        let at = match &self.doc.big {
            Some(b) => ito_openin::Pos::new(b.line(), ito_openin::Col::new(1, 1)),
            None => ed.editor_pos(),
        };
        let job = ed.is_dirty().then(|| ed.save_job().ok()).flatten();
        if let Some(job) = job {
            if !self.send_file(FileJob::Save(job)) {
                return;
            }
            self.doc.saving += 1;
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
                FileDone::Disk { path, raw } => self.disk_read(path, raw),
                FileDone::Image { path, result } => self.image_done(path, result),
                FileDone::Saved { version, path, result, bytes } => {
                    // The save belongs to the tab of that file, in front or not.
                    if let Some(d) = self.doc_mut(&path) {
                        d.saving = d.saving.saturating_sub(1);
                    }
                    let name = path.display().to_string();
                    let closes = self.close_when_saved.iter().position(|p| *p == path).map(|i| self.close_when_saved.remove(i)).is_some();
                    match result {
                        Ok(()) => {
                            if let Some(ed) = self.doc_mut(&path).and_then(|d| d.editor.as_mut()) {
                                ed.set_disk(bytes);
                                ed.saved(version);
                            }
                            // A close asked for with the save: only when nothing was typed meanwhile.
                            if closes && !self.doc_mut(&path).is_some_and(|d| d.dirty()) {
                                self.close_tab(&path);
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
    fn take_tree(&mut self, ctx: &egui::Context) {
        let Some(ed) = &self.doc.editor else { return };
        // Each ask copies the whole text, so past a size it is made at most every BIG_PAUSE (and once more when typing stops).
        let wait = if ed.text().len() > BIG_TEXT {
            self.doc.sent_at.map_or(std::time::Duration::ZERO, |t| BIG_PAUSE.saturating_sub(t.elapsed()))
        } else {
            std::time::Duration::ZERO
        };
        self.doc.worker.set_lint(&self.prefs.lint);
        if wait.is_zero() {
            self.doc.sent_at = Some(std::time::Instant::now());
            self.doc.worker.ask(ed.version(), ed.text(), ed.path().and_then(|p| p.parent()));
        } else if self.doc.worker.asked_version() != Some(ed.version()) {
            ctx.request_repaint_after(wait);
        }
        if let Some(t) = self.doc.worker.take() {
            if self.doc.tree.as_ref().is_none_or(|old| old.version <= t.version) {
                self.doc.tree = Some(t);
            }
        }
        let (base, generation) = ed.base();
        let c = if wait.is_zero() { self.doc.changes.update(ed.version(), generation, base, ed.text()) } else { self.doc.changes.newest() };
        self.doc.view.set_changes(c);
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
        out.push_str(&self.tabs_report());
        out.push_str(&format!("closing: {}\n", self.closing.is_some()));
        out.push_str(&self.find_report());
        out.push_str(&self.preview_report());
        if let Some(ed) = &self.doc.editor {
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
            // Counted here, not taken from the reading: a state must say the text as it is.
            // A file that is not UTF-8 has no count, as in the status bar: its
            // text is a lossy decoding, not what the file says.
            let text = ed.text();
            if ed.locked() != Some(engine::Locked::NotUtf8) {
                out.push_str(&format!(
                    "chars: {}\nselected_chars: {}\n",
                    count::count(text, 0..text.len()),
                    count::count(text, ed.selection().range()),
                ));
            }
            // The text on one line, as Rust writes a string, cut short.
            let text: String = ed.text().escape_debug().take(400).collect();
            out.push_str(&format!("text: {text}\n"));
        }
        out
    }

    /// `heading.fold` (the section the caret is in; the caret goes to its
    /// heading), `heading.fold_all` and `heading.unfold_all`.
    fn fold_command(&mut self, name: &str) {
        if !self.editing() {
            return;
        }
        let doc = &mut self.doc;
        let (Some(ed), Some(tree)) = (doc.editor.as_mut(), doc.tree.as_ref()) else { return };
        match name {
            "heading.fold" => {
                let Some(i) = ito_outline::at_byte(&tree.headings, ed.caret()) else { return };
                let h = &tree.headings[i];
                doc.view.folds.toggle(tree, i);
                // A caret in the body would sit in hidden lines: it goes to the heading.
                if ed.caret() > h.range.end {
                    ed.set_caret(h.range.start, false);
                }
            }
            "heading.fold_all" => {
                doc.view.folds.set_all(tree, true);
                // The outermost section holding the caret is the one left showing it.
                let caret = ed.caret();
                if let Some(h) = tree.headings.iter().find(|h| h.section.contains(&caret)) {
                    if caret > h.range.end {
                        ed.set_caret(h.range.start, false);
                    }
                }
            }
            _ => doc.view.folds.set_all(tree, false),
        }
        doc.view.follow = true;
    }

    /// `view.focus`: show only the section the caret is in (it follows the caret), or all again.
    fn focus_toggle(&mut self) {
        if !self.editing() {
            return;
        }
        let view = &mut self.doc.view;
        view.folds.set_focus(!view.folds.focused());
        view.follow = true;
    }

    /// `lint.next` and `lint.prev` (the caret goes to the finding, whose kind
    /// the status line says) and `lint.fix_all` (one undo step), by the findings
    /// of the text as it is.
    fn lint_command(&mut self, name: &str) {
        if !self.editing() {
            return;
        }
        let Some(ed) = self.doc.editor.as_mut() else { return };
        // The newest reading when it is of this text (it checked the links on the worker); else read here without touching the disk.
        let findings = match self.doc.tree.as_ref().filter(|t| t.version == ed.version()) {
            Some(t) => t.lint.clone(),
            None => lint::run(ed.text(), None, &self.prefs.lint),
        };
        let note = if name == "lint.fix_all" {
            let n = findings.iter().filter(|f| f.fix.is_some()).count();
            if n > 0 && ed.structure(|t, _| ito_mdlint::fix_all(t, &findings, None)) {
                self.table.trf("lint.fixed", &[&n.to_string()])
            } else {
                self.table.tr("lint.nofix").to_owned()
            }
        } else if let Some(f) = lint::step(&findings, ed.caret(), name == "lint.next") {
            ed.set_caret(f.range.start, false);
            self.doc.view.follow = true;
            let what = self.table.tr(&lint::key(f.kind)).to_owned();
            self.table.trf("lint.found", &[&what, &findings.len().to_string()])
        } else {
            self.table.tr("lint.none").to_owned()
        };
        self.announce(note.clone());
        self.note = Some(note);
    }

    /// The title of the section the caret is in, by the newest reading.
    fn section(&self) -> Option<String> {
        let (ed, tree) = (self.doc.editor.as_ref()?, self.doc.tree.as_ref()?);
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

    /// The bottom bar ([`App::status_items`]), the clock from common.toml at
    /// the right end.
    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let composing = self.editing() && self.doc.view.composing(ui.ctx());
        self.read_ime(ui.ctx());
        let (left, right) = self.status_items(composing);
        let clock = self.prefs.common.clock_or(None);
        let spec = ito_statusbar::Spec { clock: &clock, lang: &self.lang, opacity: 1.0 };
        ito_statusbar::show(
            ui,
            &spec,
            |ui| {
                for (s, color) in &left {
                    ito_statusbar::item(ui, s, *color);
                }
            },
            // The right side is laid out from the clock leftwards.
            |ui| {
                for (s, color) in right.iter().rev() {
                    ito_statusbar::item(ui, s, *color);
                }
            },
        );
    }

    /// The input method's state for the status bar, read each frame (see
    /// [`App::ime`]).
    fn read_ime(&mut self, ctx: &egui::Context) {
        self.ime = self.doc.view.ime_status(ctx);
    }

    /// What the status bar says, left and right, each in reading order. On
    /// the left the caret's line and column, the characters selected and in
    /// the document, the section the caret is in, what is not saved yet and
    /// the last thing done (a save, the scale); the counts are grouped by
    /// thousands (`12,345`). On the right, by the clock,
    /// the input method composing (`composing`), whether it is on and its
    /// input mode (nothing where that cannot be told) and how the file is
    /// written (its encoding, a BOM, its line ends). The document's count is the
    /// newest reading's, so no key waits for it; the selection's is counted
    /// from the reading of this text, whatever its size, or, while the
    /// reading is behind, walked only when short and otherwise pending
    /// (`count::Selected`). A file that is not UTF-8 shows no count: its text
    /// is a lossy decoding, not what the file says.
    fn status_items(&mut self, composing: bool) -> (StatusItems, StatusItems) {
        let c = ito_theme::colors();
        let mut left = StatusItems::new();
        let mut right = StatusItems::new();
        if let Some(ed) = &self.doc.editor {
            let t = &self.table;
            let (line, col) = ed.line_col();
            if let Some(b) = &self.doc.big {
                let n = ito_common::text::grouped(b.count() as u64);
                left.push((t.trf("status.big_lines", &[&ito_common::text::grouped(b.line() as u64), &n]), c.fg));
                if b.progress() < 1.0 {
                    left.push((t.trf("status.indexing", &[&((b.progress() * 100.0) as u32).to_string()]), c.wait));
                }
            } else {
                left.push((t.trf("status.line_col", &[&(line + 1).to_string(), &(col + 1).to_string()]), c.fg));
            }
            let counted = ed.locked().is_none();
            let sel = ed.selection();
            if counted && !sel.is_empty() {
                let reading = self.doc.tree.as_ref().filter(|tree| tree.version == ed.version()).map(|tree| &tree.counts);
                let n = match self.doc.selected.count(ed.version(), ed.text(), sel.range(), reading) {
                    Some(n) => ito_common::text::grouped(n as u64),
                    None => t.tr("status.pending").to_owned(),
                };
                left.push((t.trf("status.selected", &[&n]), c.fg));
            }
            if let Some(tree) = self.doc.tree.as_ref().filter(|_| counted) {
                left.push((t.trf("status.chars", &[&ito_common::text::grouped(tree.chars as u64)]), c.dim));
            }
            if let Some(s) = self.section() {
                left.push((s, c.dim));
            }
            if ed.locked().is_some() {
                left.push((t.tr("status.read_only").to_owned(), c.wait));
            } else if ed.is_dirty() {
                left.push((t.tr("status.unsaved").to_owned(), c.wait));
            }
            if self.doc.saving > 0 {
                left.push((t.tr("status.saving").to_owned(), c.dim));
            }
            if composing {
                right.push((t.tr("status.composing").to_owned(), c.fg));
            }
            if let Some(ime) = self.ime {
                // Off is the quiet state; on, typing goes through the input method.
                let (key, color) = match ime {
                    ito_ime::Status { open: false, .. } => ("status.ime_off", c.dim),
                    ito_ime::Status { mode, .. } => (ime_mode_key(mode), c.fg),
                };
                right.push((t.tr(key).to_owned(), color));
            }
            match ed.locked() {
                Some(engine::Locked::NotUtf8) => right.push((t.tr("status.not_utf8").to_owned(), c.wait)),
                Some(engine::Locked::Big) => right.push((self.doc.big.as_ref().map_or_else(String::new, big::Big::encoding), c.dim)),
                None => right.push((t.tr("status.utf8").to_owned(), c.dim)),
            }
            if ed.bom() {
                right.push((t.tr("status.bom").to_owned(), c.dim));
            }
            let eol = match ed.eol() {
                ito_textbuf::Eol::Lf => Some("status.lf"),
                ito_textbuf::Eol::Crlf => Some("status.crlf"),
                ito_textbuf::Eol::Mixed => Some("status.mixed"),
                ito_textbuf::Eol::None => None,
            };
            if let Some(k) = eol {
                right.push((t.tr(k).to_owned(), c.dim));
            }
        }
        if let Some(n) = &self.note {
            left.push((n.clone(), c.dim));
        }
        (left, right)
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
        // About a file that is a tab: that tab comes forward, over whatever covers it.
        if let Some(go) = n.go.clone().filter(|g| self.tabs.position(g).is_some()) {
            self.settings_open = false;
            self.help.close();
            self.palette = None;
            self.show_tab(&go);
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
        self.frame(ui);
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
        self.clear_recovery();
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

impl App {
    /// One frame of the window, all of it (the tests run it with the events
    /// of a real keyboard).
    fn frame(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.sync_common(&ctx);
        self.clicked();
        self.take_files(&ctx);
        self.take_external(&ctx);
        self.take_recovery(&ctx);
        self.take_folds();
        self.take_door(&ctx);
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
        if self.closing.is_some() {
            self.closing_keys(&ctx);
        }
        self.recover_keys(&ctx);
        self.external_keys(&ctx);
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
        self.tab_bar(ui);
        self.find_bar(ui);
        egui::Panel::bottom("statusbar").exact_size(ito_statusbar::HEIGHT).frame(egui::Frame::NONE).show(ui, |ui| self.status_bar(ui));
        self.take_notices();
        self.notice_list(&ctx);
        if !taking {
            self.command_palette(&ctx);
        }
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
                self.doc.view.swallow(&ctx);
            }
            let editing_big = self.editing() && self.doc.big.is_some();
            if editing_big {
                if let Some(b) = &mut self.doc.big {
                    b.keys(&ctx);
                }
            }
            self.read_keys(&ctx);
            // A key may have opened the help or the settings.
            let editing = self.editing();
            self.find_sync();
            self.preview_pane(ui);
            if let Some(big) = self.doc.big.as_mut().filter(|_| self.doc.editor.is_some()) {
                big.show(ui);
            } else if let Some(ed) = &mut self.doc.editor {
                if !editing {
                    ed.seal();
                }
                let finds: &[std::ops::Range<usize>] =
                    if self.find.open { self.doc.finder.marks(&find::key(&self.find, ed, &self.doc.finder)) } else { &[] };
                self.doc.view.show(ui, ed, self.doc.tree.as_ref(), finds, editing, self.wrap);
                self.preview_sync();
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
        self.closing_question(&ctx);
        self.external_question(&ctx);
        self.recover_question(&ctx);
        // What the screen reader hears (and `--keys`' <Announced:> checks).
        for m in std::mem::take(&mut self.said) {
            ito_a11y::announce(ui, &m);
        }
        self.take_tree(&ctx);
        self.script(&ctx);
        // Last, so the edges of the frameless window win over what is under them.
        if !cfg!(target_os = "macos") {
            ito_titlebar::resize_edges(&ctx);
        }
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
        if let Some(ed) = &mut app.doc.editor {
            ed.go_to(self.line, self.col);
            app.doc.view.follow = true;
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

/// More files after the first (`weft a.md b.md`): each plain argument up to the
/// first option. They open as tabs behind the first.
fn more_files(args: &[String]) -> (Vec<PathBuf>, &[String]) {
    let n = args.iter().take_while(|a| !a.starts_with('-') && !a.starts_with('+') && *a != "env").count();
    (args[..n].iter().map(PathBuf::from).collect(), &args[n..])
}

fn main() -> eframe::Result {
    let all: Vec<String> = std::env::args().skip(1).collect();
    let done = std::env::var_os("WEFT_KEYS_DONE").filter(|v| !v.is_empty()).map(PathBuf::from);
    let parsed = file_arg(&all).and_then(|(file, goto, args)| {
        let (more, args) = if file.is_some() { more_files(args) } else { (Vec::new(), args) };
        keys_arg(args).map(|script| (file, goto, more, args, script))
    });
    let (file, goto, more, args, script) = match parsed {
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
    // With `open_from_outside = new_tab` one window serves every `weft FILE`:
    // a later launch hands its file over and goes away. A script never does.
    let ctx_cell = std::sync::Arc::new(std::sync::OnceLock::<egui::Context>::new());
    let mut door = None;
    if script.is_none() && prefs::tabs_now(ito_common::base_dir().as_deref()).open_from_outside == ito_tabs::OpenFromOutside::NewTab {
        let cwd = std::env::current_dir().unwrap_or_default();
        match door::acquire(&door::request(file.as_deref(), goto, &more, &cwd), ctx_cell.clone()) {
            door::Door::Forwarded => return Ok(()),
            door::Door::Owner(o) => door = Some(o),
            door::Door::Closed => {}
        }
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
    let file_front = file.clone();
    eframe::run_native(
        "weft",
        options,
        Box::new(move |cc| {
            let script = script.map(|steps| {
                let mut r = Runner::new(steps, done);
                r.watch(cc.egui_ctx.clone());
                r
            });
            let _ = ctx_cell.set(cc.egui_ctx.clone());
            let mut app = App::new(cc, &saved, script, file);
            app.door = door;
            // The first file stays in front; the others wait behind it.
            if let Some(first) = file_front.filter(|_| !more.is_empty()) {
                for m in &more {
                    app.open(m);
                }
                app.show_tab(&first);
            }
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
            assert!(app.doc.view.follow, "the first frame scrolls to it");
        }
        let all = strings(&["+40", &name]);
        let (path, goto, _) = file_arg(&all).unwrap();
        let mut app = App::for_tests();
        app.open(&path.unwrap());
        goto.unwrap().apply(&mut app);
        assert!(app.report().contains("caret: 4:1\n"), "past the end is the last line: {}", app.report());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 9.7: Plain arguments after the first file are more files, up to the first option.
    #[test]
    fn plain_arguments_after_the_file_are_more_files() {
        let v = strings(&["b.md", "c.md", "--keys", "j"]);
        let (more, rest) = more_files(&v);
        assert_eq!(more, vec![PathBuf::from("b.md"), PathBuf::from("c.md")]);
        assert_eq!(rest, &v[2..]);
        assert!(more_files(&strings(&["--version"])).0.is_empty());
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
        app.doc.tree = Some(structure::read(ed.version(), ed.text()));
        app.doc.editor = Some(ed);
        assert_eq!(app.section().as_deref(), Some("Two"));
    }

    /// 8.9: The status bar counts the characters of the newest reading (line
    /// breaks left out, `👍🏽` and `é` one each) and, while something is
    /// selected, the selected ones; it says the encoding, the line ends and
    /// the input method composing; a `--keys` state says the counts too.
    #[test]
    fn the_status_bar_counts_the_characters_and_says_how_the_file_is_written() {
        let words = |items: &StatusItems| items.iter().map(|(s, _)| s.clone()).collect::<Vec<_>>();
        let text = "# 見出し\r\n\r\n👍🏽 e\u{301}\r\n";
        let mut app = App::for_tests();
        app.doc.editor = Some(engine::Editor::new(text));
        let (left, right) = app.status_items(false);
        assert_eq!(words(&left), ["Ln 1, Col 1"], "no reading yet, so no count");
        assert_eq!(words(&right), ["UTF-8", "CRLF"]);

        let ed = app.doc.editor.as_ref().unwrap();
        app.doc.tree = Some(structure::read(ed.version(), ed.text()));
        let (left, right) = app.status_items(true);
        assert_eq!(words(&left), ["Ln 1, Col 1", "Chars 8", "見出し"]);
        assert_eq!(words(&right), ["Composing", "UTF-8", "CRLF"]);

        let ed = app.doc.editor.as_mut().unwrap();
        ed.set_caret(text.find('e').unwrap(), true);
        let (left, _) = app.status_items(false);
        assert!(words(&left).contains(&"Selected 7".to_owned()), "{left:?}");
        let ed = app.doc.editor.as_mut().unwrap();
        ed.set_caret(0, false);
        ed.set_caret(text.len(), true);
        let (left, _) = app.status_items(false);
        assert!(words(&left).contains(&"Selected 8".to_owned()), "{left:?}");
        assert!(app.report().contains("chars: 8\nselected_chars: 8\n"), "{}", app.report());

        // A typed key: the bar keeps the old reading's count until a new one comes.
        let ed = app.doc.editor.as_mut().unwrap();
        ed.set_caret(0, false);
        ed.insert("ab", 0);
        let (left, _) = app.status_items(false);
        assert!(words(&left).contains(&"Chars 8".to_owned()), "{left:?}");
        assert!(app.report().contains("chars: 10\nselected_chars: 0\n"), "{}", app.report());

        // A file that is not UTF-8 says so, read-only.
        let dir = std::env::temp_dir().join(format!("weft-status-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("sjis.md");
        std::fs::write(&file, b"\x82\xa0\n").unwrap();
        let mut app = App::for_tests();
        app.open(&file);
        let (left, right) = app.status_items(false);
        assert!(words(&left).contains(&"Read-only".to_owned()), "{left:?}");
        assert_eq!(words(&right), ["Not UTF-8", "LF"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 21.x: text a crashed run had not saved comes back as unsaved changes
    /// (one undo step), or is left.
    #[test]
    fn unsaved_text_of_a_crashed_run_is_restored_or_left() {
        let dir = std::env::temp_dir().join(format!("weft-recover-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.md");
        std::fs::write(&file, "one\ntwo\n").unwrap();
        let mut app = App::for_tests();
        app.recover = vec![recovery::Entry { path: file.clone(), text: "one\ntwo\nthree\n".to_owned() }];
        assert!(app.recover_open());
        app.answer_recover(1);
        assert!(!app.recover_open());
        let ed = app.doc.editor.as_mut().unwrap();
        assert_eq!(ed.text(), "one\ntwo\nthree\n");
        assert!(ed.is_dirty(), "still not saved");
        assert!(ed.undo());
        assert_eq!(ed.text(), "one\ntwo\n");
        // Left: the file stays as it is.
        let mut app = App::for_tests();
        app.recover = vec![recovery::Entry { path: file.clone(), text: "x\n".to_owned() }];
        app.answer_recover(0);
        assert!(!app.recover_open());
        assert!(app.doc.editor.is_none());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "one\ntwo\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 19.x: a file changed by someone else is followed when the window has no
    /// unsaved changes, and asked about when it has.
    #[test]
    fn an_external_change_is_followed_when_clean_and_asked_about_when_dirty() {
        let dir = std::env::temp_dir().join(format!("weft-external-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.md");
        std::fs::write(&file, "one\ntwo\n").unwrap();
        let mut app = App::for_tests();
        app.open(&file);
        // The same text: nothing.
        app.disk_read(file.clone(), Some("one\ntwo\n".to_owned()));
        assert!(app.asked.is_empty());
        // Clean: followed, as one undo step.
        app.disk_read(file.clone(), Some("one\nTWO\n".to_owned()));
        assert!(app.asked.is_empty());
        let ed = app.doc.editor.as_mut().unwrap();
        assert_eq!(ed.text(), "one\nTWO\n");
        assert!(!ed.is_dirty());
        assert!(ed.undo());
        // Dirty: asked, and a later change replaces the earlier question.
        let ed = app.doc.editor.as_mut().unwrap();
        ed.replace(&[(0..0, "mine ".to_owned())], ito_textbuf::Selection { anchor: 5, head: 5 });
        app.disk_read(file.clone(), Some("one\nthree\n".to_owned()));
        app.disk_read(file.clone(), Some("one\nfour\n".to_owned()));
        assert_eq!(app.asked.len(), 1);
        assert_eq!(app.asked[0].disk, "one\nfour\n");
        assert_eq!(app.doc.editor.as_ref().unwrap().text(), "mine one\ntwo\n", "nothing taken yet");
        // Keep: not asked again about the same contents.
        app.answer_external(0);
        app.disk_read(file.clone(), Some("one\nfour\n".to_owned()));
        assert!(app.asked.is_empty());
        // A different change asks again; taking it is one undo step and stays changed.
        app.disk_read(file.clone(), Some("one\nfive\n".to_owned()));
        assert_eq!(app.asked.len(), 1);
        app.answer_external(1);
        let ed = app.doc.editor.as_mut().unwrap();
        assert_eq!(ed.text(), "one\nfive\n");
        assert!(ed.is_dirty());
        assert!(ed.undo());
        assert_eq!(ed.text(), "mine one\ntwo\n");
        // Typing what the file now holds is not a conflict.
        let ed = app.doc.editor.as_mut().unwrap();
        ed.reload("mine one\nsix\n", false);
        app.disk_read(file.clone(), Some("mine one\nsix\n".to_owned()));
        assert!(app.asked.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 8.14: by the clock, after "Composing" and before the encoding, the
    /// status bar says whether the input method is on and, on, its input
    /// mode, in the language's words; where that cannot be told, and with no
    /// document, nothing.
    #[test]
    fn the_status_bar_says_whether_the_input_method_is_on_and_its_mode() {
        use ito_ime::{Ime, Mode, Status};
        let words = |items: &StatusItems| items.iter().map(|(s, _)| s.clone()).collect::<Vec<_>>();
        let ctx = egui::Context::default();
        let mut app = App::for_tests();
        Ime::fake_status(&ctx, Some(Some(Status { open: true, mode: Mode::Hiragana })));
        app.read_ime(&ctx);
        assert!(app.status_items(false).1.is_empty(), "no document, no item");

        app.doc.editor = Some(engine::Editor::new("a\n"));
        let on = |open, mode| Some(Status { open, mode });
        for (status, want) in [
            (on(false, Mode::HalfAlnum), "IME off"),
            (on(true, Mode::Hiragana), "Hiragana"),
            (on(true, Mode::FullKatakana), "Full-width katakana"),
            (on(true, Mode::HalfKatakana), "Half-width katakana"),
            (on(true, Mode::FullAlnum), "Full-width alphanumeric"),
            (on(true, Mode::HalfAlnum), "Half-width alphanumeric"),
            (on(true, Mode::Other), "IME on"),
        ] {
            Ime::fake_status(&ctx, Some(status));
            app.read_ime(&ctx);
            let (_, right) = app.status_items(true);
            assert_eq!(words(&right), ["Composing", want, "UTF-8", "LF"], "{status:?}");
            let c = ito_theme::colors();
            let color = if status.is_some_and(|s| s.open) { c.fg } else { c.dim };
            assert_eq!(right[1].1, color, "{status:?}: on in the text's colour, off quieter");
        }

        Ime::fake_status(&ctx, Some(None));
        app.read_ime(&ctx);
        assert_eq!(words(&app.status_items(false).1), ["UTF-8", "LF"], "cannot tell: nothing");

        // In Japanese.
        let (table, _) = i18n::table("ja");
        app.table = table;
        for (status, want) in [
            (on(false, Mode::HalfAlnum), "IME オフ"),
            (on(true, Mode::Hiragana), "ひらがな"),
            (on(true, Mode::FullKatakana), "全角カナ"),
            (on(true, Mode::HalfKatakana), "半角カナ"),
            (on(true, Mode::FullAlnum), "全角英数"),
            (on(true, Mode::HalfAlnum), "半角英数"),
            (on(true, Mode::Other), "IME オン"),
        ] {
            Ime::fake_status(&ctx, Some(status));
            app.read_ime(&ctx);
            assert_eq!(words(&app.status_items(false).1)[0], want, "{status:?}");
        }
    }

    /// 8.9: A file that is not UTF-8 shows no count, in the bar or in a
    /// `--keys` state, even with a reading and a selection: its text is a
    /// lossy decoding, which reads `あい` in Shift_JIS as more characters
    /// than it has.
    #[test]
    fn a_file_that_is_not_utf8_shows_no_count() {
        let words = |items: &StatusItems| items.iter().map(|(s, _)| s.clone()).collect::<Vec<_>>();
        let dir = std::env::temp_dir().join(format!("weft-sjis-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("sjis.md");
        // `あい\n# 見出し\n` in Shift_JIS: 7 characters.
        std::fs::write(&file, b"\x82\xa0\x82\xa2\n# \x8c\xa9\x8f\x6f\x82\xb5\n").unwrap();
        let mut app = App::for_tests();
        app.open(&file);
        let ed = app.doc.editor.as_mut().unwrap();
        assert_eq!(ed.locked(), Some(engine::Locked::NotUtf8));
        ed.select_all();
        app.doc.tree = Some(structure::read(ed.version(), ed.text()));
        let (left, right) = app.status_items(false);
        let left = words(&left);
        assert!(!left.iter().any(|s| s.starts_with("Chars") || s.starts_with("Selected")), "{left:?}");
        assert!(left.contains(&"Read-only".to_owned()), "{left:?}");
        assert_eq!(words(&right), ["Not UTF-8", "LF"]);
        let report = app.report();
        assert!(report.contains("locked: true\n"), "{report}");
        assert!(!report.contains("chars:"), "{report}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 8.9: A large selection's count comes from the reading of the text as
    /// it is, without walking the selection on the UI thread; while the
    /// reading is behind (Ctrl+A before it comes, an undo that brings a
    /// large selection back) the bar says the count is pending, and walks
    /// nothing either.
    #[test]
    fn a_large_selection_is_counted_from_the_reading_or_shown_pending() {
        let words = |items: &StatusItems| items.iter().map(|(s, _)| s.clone()).collect::<Vec<_>>();
        let line = "# 見出し\r\n\r\n日本語の文。👍🏽 e\u{301} 👨‍👩‍👧\r\n";
        let text = line.repeat((256 << 10) / line.len() + 1);
        // A reading of the text as it is now, as the worker makes it (with no headings or colours).
        let reading = |app: &App| {
            let ed = app.doc.editor.as_ref().unwrap();
            let counts = count::Index::new(ed.text());
            let (version, chars) = (ed.version(), counts.total());
            structure::Tree {
                version,
                headings: Vec::new(),
                lines: Vec::new(),
                rows: Vec::new(),
                breaks: Vec::new(),
                chars,
                counts,
                lint: Vec::new(),
            }
        };
        let near = 2 * (count::STEP + 64);
        let mut app = App::for_tests();
        app.doc.editor = Some(engine::Editor::new(&text));
        app.doc.editor.as_mut().unwrap().select_all();
        let ((left, _), walk) = count::walked(|| app.status_items(false));
        assert!(words(&left).contains(&"Selected …".to_owned()), "Ctrl+A before the reading: {left:?}");
        assert_eq!(walk, 0);

        app.doc.tree = Some(reading(&app));
        let ((left, _), walk) = count::walked(|| app.status_items(false));
        let all = count::count(&text, 0..text.len());
        assert!(words(&left).contains(&format!("Selected {}", ito_common::text::grouped(all as u64))), "the reading came: {left:?}");
        assert!(walk <= near, "walked {walk} bytes");

        let ed = app.doc.editor.as_mut().unwrap();
        ed.set_caret(line.len() + 2, false);
        ed.move_caret(engine::Motion::DocEnd, true);
        let ((left, _), walk) = count::walked(|| app.status_items(false));
        let n = count::count(&text, line.len() + 2..text.len());
        assert!(words(&left).contains(&format!("Selected {}", ito_common::text::grouped(n as u64))), "Ctrl+Shift+End: {left:?}");
        assert!(walk <= near, "walked {walk} bytes");

        // Typed over, then undone: the selection is back, the reading behind.
        let ed = app.doc.editor.as_mut().unwrap();
        assert!(ed.insert("x", 0));
        assert!(ed.undo());
        assert_eq!(ed.selection().range(), line.len() + 2..text.len());
        assert_ne!(Some(ed.version()), app.doc.tree.as_ref().map(|t| t.version));
        let ((left, _), walk) = count::walked(|| app.status_items(false));
        assert!(words(&left).contains(&"Selected …".to_owned()), "an edit with the selection kept: {left:?}");
        assert_eq!(walk, 0);
        app.doc.tree = Some(reading(&app));
        let (left, _) = app.status_items(false);
        assert!(words(&left).contains(&format!("Selected {}", ito_common::text::grouped(n as u64))), "{left:?}");
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
        app.doc.editor = Some(ed);
        app.quit(&ctx);
        app.answer_quit(&ctx, 1);
        assert_eq!(app.quit, Some(Quit::Go), "N quits as it is");
        assert_eq!(app.doc.saving, 0, "and writes nothing");

        let mut app = App::for_tests();
        let mut ed = engine::Editor::new("a");
        ed.insert("b", 0);
        app.doc.editor = Some(ed);
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
        app.doc.editor.as_mut().unwrap().insert("b", 0);
        app.quit(&ctx);
        app.answer_quit(&ctx, 2);
        assert_eq!(app.quit, Some(Quit::Saving));
        let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while app.doc.saving > 0 && std::time::Instant::now() < until {
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
        app.doc.editor = Some(engine::Editor::new("a"));
        app.run_key(&ctx, keymap::Cmd::Run("edit.undo"));
        assert_eq!(app.said, ["Undo: nothing to do here"]);
        app.run_key(&ctx, keymap::Cmd::Run("cursor.left"));
        assert_eq!(app.said.len(), 1, "a move at the start is not news");
        app.run_key(&ctx, keymap::Cmd::Run("file.save"));
        assert_eq!(app.said.len(), 2);
        assert!(app.said[1].starts_with("This document has no file yet"), "{:?}", app.said);
        assert_eq!(app.notices.len(), 1, "and it is under the bell");

        app.said.clear();
        app.doc.editor.as_mut().unwrap().insert("b", 0);
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
        app.doc.editor.as_mut().unwrap().insert("b", 0);
        app.save();
        let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while app.doc.saving > 0 && std::time::Instant::now() < until {
            std::thread::sleep(std::time::Duration::from_millis(10));
            app.take_files(&ctx);
        }
        assert_eq!(app.said, ["Saved s.md"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 10.10: Bold next to punctuation (`**「強調」**です`) goes in, and one
    /// line on the status bar, said to the screen reader, tells that it may
    /// not show as bold: nothing under the bell, nothing to answer. Taking
    /// it off says nothing.
    #[test]
    fn bold_next_to_punctuation_is_put_in_and_told_on_the_status_bar() {
        let ctx = egui::Context::default();
        let mut app = App::for_tests();
        let mut ed = engine::Editor::new("「強調」です");
        ed.set_caret(0, false);
        ed.set_caret("「強調」".len(), true);
        app.doc.editor = Some(ed);
        app.run_key(&ctx, keymap::Cmd::Run("inline.bold"));
        assert_eq!(app.doc.editor.as_ref().unwrap().text(), "**「強調」**です");
        let line = "The marks are in, but next to punctuation they may not show as emphasis";
        assert_eq!(app.note.as_deref(), Some(line));
        assert_eq!(app.said, [line]);
        assert!(app.notices.is_empty(), "not under the bell");
        assert!(app.quit.is_none() && app.palette.is_none(), "nothing to answer");

        app.note = None;
        app.run_key(&ctx, keymap::Cmd::Run("inline.bold"));
        assert_eq!(app.doc.editor.as_ref().unwrap().text(), "「強調」です");
        assert_eq!(app.note, None);
        assert_eq!(app.said.len(), 1);
    }

    // ---- The keys as a Windows keyboard sends them, through whole frames.

    use egui::{Event, Key as K, Modifiers as M};

    /// Ctrl as egui-winit reports it on Windows: `ctrl` and `command` both.
    const CTRL: M = M { ctrl: true, command: true, ..M::NONE };

    /// A key pressed or let go, as egui-winit sends it (the physical key is
    /// not read).
    fn key(k: K, modifiers: M, pressed: bool) -> Event {
        Event::Key { key: k, physical_key: Some(k), pressed, repeat: false, modifiers }
    }

    /// One stroke as egui-winit sends it on Windows: Ctrl, Alt or Shift
    /// going down, the press, the character it types (an Alt chord's too; a
    /// Ctrl chord's is dropped there, so none), the release, then the
    /// modifiers let go.
    fn stroke(k: K, mods: M, text: Option<&str>) -> Vec<Event> {
        let mut v = Vec::new();
        if mods != M::NONE {
            v.push(Event::ModifiersChanged(mods));
        }
        v.push(key(k, mods, true));
        v.extend(text.map(|t| Event::Text(t.to_owned())));
        v.push(key(k, mods, false));
        if mods != M::NONE {
            v.push(Event::ModifiersChanged(M::NONE));
        }
        v
    }

    /// A clipboard key as egui-winit sends it on Windows: the modifier
    /// going down, the clipboard's event in place of the key's press, the
    /// key's release, the modifier let go.
    fn clip(event: Event, k: K, mods: M) -> Vec<Event> {
        vec![Event::ModifiersChanged(mods), event, key(k, mods, false), Event::ModifiersChanged(M::NONE)]
    }

    /// An app with `text` open and `from..to` selected (the caret at `to`),
    /// and the context its frames run in.
    fn typing_into(text: &str, from: usize, to: usize) -> (App, egui::Context) {
        let mut app = App::for_tests();
        let mut ed = engine::Editor::new(text);
        ed.set_caret(from, false);
        ed.set_caret(to, true);
        app.doc.editor = Some(ed);
        (app, egui::Context::default())
    }

    /// One whole frame of the app with `events`.
    fn frame_with(app: &mut App, ctx: &egui::Context, events: Vec<Event>) -> egui::FullOutput {
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0))),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(raw, |ui| app.frame(ui));
        // No painter takes the font's textures here.
        out.textures_delta.clear();
        out
    }

    /// `cargo test -p weft --release -- --ignored --nocapture big_text_read`
    /// with `WEFT_BIG_MD=path`: how long the structure worker's whole reading takes.
    #[test]
    #[ignore]
    fn big_text_read() {
        let Ok(path) = std::env::var("WEFT_BIG_MD") else { return };
        let text = std::fs::read_to_string(&path).unwrap();
        for _ in 0..3 {
            let t = std::time::Instant::now();
            let tree = crate::structure::read(1, &text);
            eprintln!("read {} MiB: {:?} ({} headings)", text.len() >> 20, t.elapsed(), tree.headings.len());
        }
    }

    /// `cargo test -p weft --release -- --ignored --nocapture big_text_frames`
    /// with `WEFT_BIG_MD=path`: how long a whole frame takes while typing at
    /// the end of, and in the middle of, a big file (TODO.md, big files).
    #[test]
    #[ignore]
    fn big_text_frames() {
        let Ok(path) = std::env::var("WEFT_BIG_MD") else { return };
        let text = std::fs::read_to_string(&path).unwrap();
        for (label, at) in [("end", text.len()), ("middle", text.len() / 2)] {
            let at = (0..=at).rev().find(|&i| text.is_char_boundary(i)).unwrap();
            let (mut app, ctx) = typing_into(&text, at, at);
            frame_with(&mut app, &ctx, vec![]);
            let mut worst = std::time::Duration::ZERO;
            let mut total = std::time::Duration::ZERO;
            for _ in 0..30 {
                let t = std::time::Instant::now();
                frame_with(&mut app, &ctx, vec![Event::Text("x".to_owned())]);
                let d = t.elapsed();
                total += d;
                worst = worst.max(d);
            }
            println!("{} KiB, typing at the {label}: mean {:?}, worst {:?}", text.len() / 1024, total / 30, worst);
        }
    }

    /// A big text is read again at most every `BIG_PAUSE`, and once more when typing stops.
    #[test]
    fn a_big_text_is_read_again_after_a_pause() {
        let text = "# Title

line
"
        .repeat(BIG_TEXT / 8);
        let (mut app, ctx) = typing_into(&text, 0, 0);
        frame_with(&mut app, &ctx, vec![]);
        frame_with(&mut app, &ctx, vec![Event::Text("x".to_owned())]);
        let v = app.doc.editor.as_ref().unwrap().version();
        assert_ne!(app.doc.worker.asked_version(), Some(v), "inside the pause the text is not copied");
        std::thread::sleep(BIG_PAUSE + std::time::Duration::from_millis(20));
        frame_with(&mut app, &ctx, vec![]);
        assert_eq!(app.doc.worker.asked_version(), Some(v), "after it the newest text is asked");
    }

    fn text_of(app: &App) -> &str {
        app.doc.editor.as_ref().unwrap().text()
    }

    /// What `name` makes of `text` with `from..to` selected, run by name as
    /// the palette runs it.
    fn by_name(text: &str, from: usize, to: usize, name: &str) -> String {
        let mut ed = engine::Editor::new(text);
        ed.set_caret(from, false);
        ed.set_caret(to, true);
        assert_eq!(commands::edit(&mut ed, name, 0, 20), Some(true), "{name} changes {text:?}");
        ed.text().to_owned()
    }

    /// The palette and the key help show `name` by its key, `words` (`k`
    /// with `mods` held), and that key is not one of weft's own and is bound
    /// once.
    fn shown_and_bound_once(app: &App, name: &str, words: &str, k: K, mods: M) {
        assert_eq!(app.keys.label(name), words, "the palette shows {name}'s key");
        let desc = app.table.tr_desc(commands::find(name).unwrap().desc).to_owned();
        assert!(app.commands().iter().any(|e| e.key == words && e.title == desc), "the palette lists {words}");
        let listed = help::groups(app).into_iter().flat_map(|g| g.rows).any(|(k, d)| d == desc && k.split(" / ").any(|k| k == words));
        assert!(listed, "the key help lists {words} as {desc}");
        let pressed = ito_keys::from_egui(k, &mods).unwrap();
        assert!(keymap::Keys::fixed(&[pressed]).is_none(), "{words} is one of weft's own keys");
        let bound = keymap::Keys::built_in_layers().iter().flat_map(|(_, b)| b).filter(|b| b.on == [pressed]).count();
        assert_eq!(bound, 1, "{words} is bound {bound} times");
    }

    /// The input method's composition as egui sends it.
    fn ime_preedit(t: &str) -> Event {
        Event::Ime(egui::ImeEvent::Preedit { text: t.to_owned(), active_range_chars: None })
    }

    /// What the input method committed, as egui sends it.
    fn ime_commit(t: &str) -> Event {
        Event::Ime(egui::ImeEvent::Commit(t.to_owned()))
    }

    /// What the clipboard was given in a frame.
    fn copied(out: &egui::FullOutput) -> Vec<String> {
        out.platform_output
            .commands
            .iter()
            .filter_map(|c| match c {
                egui::OutputCommand::CopyText(t) => Some(t.clone()),
                _ => None,
            })
            .collect()
    }

    /// 10.12: Each format key, as egui sends it from a JIS and a US
    /// keyboard (Ctrl with no character), runs its command through a whole
    /// frame while the document has the keys, as the palette runs it by
    /// name, and types nothing.
    #[test]
    fn the_format_keys_run_their_commands_from_a_jis_and_a_us_keyboard() {
        let ctrl_shift = CTRL | M::SHIFT;
        let (word, lines) = ("word", "a\nb");
        // (the key egui reports, what is held, the command, the text it works on, all selected)
        let keys = [
            (K::B, CTRL, "inline.bold", word),
            (K::I, CTRL, "inline.italic", word),
            // JIS Ctrl+Shift+@, then US Ctrl+`.
            (K::Backtick, ctrl_shift, "inline.code", word),
            (K::Backtick, CTRL, "inline.code", word),
            (K::K, CTRL, "inline.link", word),
            (K::Num1, CTRL, "heading.set_1", word),
            (K::Num2, CTRL, "heading.set_2", word),
            (K::Num3, CTRL, "heading.set_3", word),
            (K::Num4, CTRL, "heading.set_4", word),
            (K::Num5, CTRL, "heading.set_5", word),
            (K::Num6, CTRL, "heading.set_6", word),
            // JIS Ctrl+@ and Ctrl+[, and US Ctrl+[, are all this.
            (K::OpenBracket, CTRL, "block.fence", lines),
            // Ctrl+Shift+] and Ctrl+Shift+[ (JIS and US): the shift is in the symbol.
            (K::CloseCurlyBracket, ctrl_shift, "list.bullet", lines),
            (K::CloseCurlyBracket, CTRL, "list.bullet", lines),
            (K::OpenCurlyBracket, ctrl_shift, "list.number", lines),
            (K::OpenCurlyBracket, CTRL, "list.number", lines),
        ];
        for (k, mods, name, text) in keys {
            let (mut app, ctx) = typing_into(text, 0, text.len());
            frame_with(&mut app, &ctx, stroke(k, mods, None));
            assert_eq!(text_of(&app), by_name(text, 0, text.len(), name), "{k:?} with {mods:?} runs {name}");
            // The chord let go in the next frame: the same, once.
            let (mut app, ctx) = typing_into(text, 0, text.len());
            frame_with(&mut app, &ctx, vec![Event::ModifiersChanged(mods), key(k, mods, true)]);
            frame_with(&mut app, &ctx, vec![key(k, mods, false), Event::ModifiersChanged(M::NONE)]);
            assert_eq!(text_of(&app), by_name(text, 0, text.len(), name), "{k:?} with {mods:?} over two frames");
        }
    }

    /// 10.14: Alt+Q, Alt+T, Alt+Shift+5, Alt+X and Alt+Shift+X, as Windows
    /// sends them (the key, then its character), run the quote, the task
    /// list, strikethrough, Check and Uncheck the selected tasks as the
    /// palette runs them by name, and type nothing; the palette and the key
    /// help show them, and none is one of weft's own keys or bound twice.
    /// Alt+5 (a key of its own now) and Alt+Z run nothing and type nothing.
    #[test]
    fn the_alt_format_keys_run_their_commands_and_type_nothing() {
        let (alt, alt_shift) = (M::ALT, M::ALT | M::SHIFT);
        // (the text, all selected; the key egui reports, what is held, the
        // character Windows sends; the command; its key in words)
        let cases = [
            ("a\nb", K::Q, alt, "q", "block.quote", "Alt+Q"),
            ("a\nb", K::T, alt, "t", "list.task", "Alt+T"),
            ("word", K::Num5, alt_shift, "%", "inline.strike", "Alt+Shift+5"),
            ("- [ ] a\n- [x] b", K::X, alt, "x", "list.check_tasks", "Alt+X"),
            ("- [x] a\n- [ ] b", K::X, alt_shift, "X", "list.uncheck_tasks", "Alt+Shift+X"),
        ];
        for (text, k, mods, typed, name, words) in cases {
            let (mut app, ctx) = typing_into(text, 0, text.len());
            frame_with(&mut app, &ctx, stroke(k, mods, Some(typed)));
            let want = by_name(text, 0, text.len(), name);
            assert_eq!(text_of(&app), want, "{k:?} with {mods:?} on {text:?} runs {name} and types no {typed:?}");
            shown_and_bound_once(&app, name, words, k, mods);
        }
        // An Alt chord that runs nothing types nothing either.
        let (mut app, ctx) = typing_into("word", 4, 4);
        frame_with(&mut app, &ctx, stroke(K::Z, alt, Some("z")));
        assert_eq!(text_of(&app), "word");
        let (mut app, ctx) = typing_into("word", 0, 4);
        frame_with(&mut app, &ctx, stroke(K::Num5, alt, Some("5")));
        assert_eq!(text_of(&app), "word", "Alt+5 is not strikethrough and types no 5");
    }

    /// 8.16: Keys and typing in one frame go in the order they came, and a
    /// key the keymap runs types nothing: `a`, Left, `b`, Alt+T, `c` make
    /// `- [ ] bca`; the undos take off `c`, the task, then the typing.
    #[test]
    fn keys_and_typing_in_one_frame_go_in_their_order() {
        let (mut app, ctx) = typing_into("", 0, 0);
        let mut events = stroke(K::A, M::NONE, Some("a"));
        events.extend(stroke(K::ArrowLeft, M::NONE, None));
        events.extend(stroke(K::B, M::NONE, Some("b")));
        events.extend(stroke(K::T, M::ALT, Some("t")));
        events.extend(stroke(K::C, M::NONE, Some("c")));
        frame_with(&mut app, &ctx, events);
        assert_eq!(text_of(&app), "- [ ] bca");
        let ed = app.doc.editor.as_mut().unwrap();
        assert!(ed.undo());
        assert_eq!(ed.text(), "- [ ] ba");
        assert!(ed.undo());
        assert_eq!(ed.text(), "ba");
        while ed.undo() {}
        assert_eq!(ed.text(), "");
    }

    /// 8.16: Plain keys, Shift and Space type their character once; Enter is
    /// a key and breaks the line.
    #[test]
    fn plain_keys_type_their_character_once() {
        let (mut app, ctx) = typing_into("", 0, 0);
        let mut events = stroke(K::A, M::NONE, Some("a"));
        events.extend(stroke(K::A, M::SHIFT, Some("A")));
        events.extend(stroke(K::Space, M::NONE, Some(" ")));
        events.extend(stroke(K::Enter, M::NONE, None));
        events.extend(stroke(K::Num1, M::NONE, Some("1")));
        events.extend(stroke(K::Num1, M::SHIFT, Some("!")));
        frame_with(&mut app, &ctx, events);
        assert_eq!(text_of(&app), "aA \n1!");
    }

    /// 8.17: Ctrl+V and Shift+Insert paste once, Ctrl+X and Shift+Delete cut
    /// once, Ctrl+C and Ctrl+Insert copy (as Windows sends them: the
    /// clipboard's event and the key's release, no press), whether the key
    /// is let go in the frame or after it.
    #[test]
    fn the_clipboard_keys_paste_cut_and_copy_once() {
        for (k, mods) in [(K::V, CTRL), (K::Insert, M::SHIFT)] {
            let (mut app, ctx) = typing_into("ab", 1, 1);
            frame_with(&mut app, &ctx, clip(Event::Paste("clip".to_owned()), k, mods));
            assert_eq!(text_of(&app), "aclipb", "{k:?} with {mods:?}");
            let (mut app, ctx) = typing_into("ab", 1, 1);
            frame_with(&mut app, &ctx, vec![Event::ModifiersChanged(mods), Event::Paste("clip".to_owned())]);
            frame_with(&mut app, &ctx, vec![key(k, mods, false), Event::ModifiersChanged(M::NONE)]);
            assert_eq!(text_of(&app), "aclipb", "{k:?} with {mods:?} let go after the frame");
        }
        for (k, mods) in [(K::X, CTRL), (K::Delete, M::SHIFT)] {
            let (mut app, ctx) = typing_into("a word", 2, 6);
            let out = frame_with(&mut app, &ctx, clip(Event::Cut, k, mods));
            assert_eq!(text_of(&app), "a ", "{k:?} with {mods:?}");
            assert_eq!(copied(&out), ["word"]);
        }
        for (k, mods) in [(K::C, CTRL), (K::Insert, CTRL)] {
            let (mut app, ctx) = typing_into("a word", 2, 6);
            let out = frame_with(&mut app, &ctx, clip(Event::Copy, k, mods));
            assert_eq!(text_of(&app), "a word", "{k:?} with {mods:?}");
            assert_eq!(copied(&out), ["word"]);
        }
    }

    /// 8.18: A read-only file (not UTF-8) takes no typing, no paste, no cut
    /// and no format key; copy still copies.
    #[test]
    fn a_read_only_file_takes_no_typing_from_the_keys() {
        let dir = std::env::temp_dir().join(format!("weft-keys-ro-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("sjis.md");
        std::fs::write(&file, b"\x82\xa0 word\n").unwrap();
        let mut app = App::for_tests();
        app.open(&file);
        let ctx = egui::Context::default();
        let was = text_of(&app).to_owned();
        let ed = app.doc.editor.as_mut().unwrap();
        ed.set_caret(0, false);
        ed.set_caret(was.len(), true);
        let mut events = stroke(K::A, M::NONE, Some("a"));
        events.extend(stroke(K::Q, M::ALT, Some("q")));
        events.extend(stroke(K::B, CTRL, None));
        events.extend(clip(Event::Paste("clip".to_owned()), K::V, CTRL));
        events.extend(clip(Event::Cut, K::X, CTRL));
        frame_with(&mut app, &ctx, events);
        assert_eq!(text_of(&app), was);
        let out = frame_with(&mut app, &ctx, clip(Event::Copy, K::C, CTRL));
        assert_eq!(copied(&out), [was]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 8.20: What the input method commits goes in once, where it comes
    /// among the keys of its frame: a Korean syllable committed by Space
    /// gives `한 ` (and is shown as a composition, not typed, before that),
    /// a commit then a key gives `日本a`, a key then a commit `a日本`, and
    /// the commit is an undo step of its own.
    #[test]
    fn the_input_methods_commits_go_in_among_the_keys_in_their_order() {
        let (mut app, ctx) = typing_into("", 0, 0);
        frame_with(&mut app, &ctx, vec![ime_preedit("한")]);
        assert_eq!(text_of(&app), "", "a composition is not typed");
        assert!(app.doc.view.composing(&ctx), "the composition is shown");
        let mut events = vec![ime_preedit(""), ime_commit("한")];
        events.extend(stroke(K::Space, M::NONE, Some(" ")));
        frame_with(&mut app, &ctx, events);
        assert_eq!(text_of(&app), "한 ");
        assert!(!app.doc.view.composing(&ctx));
        // The frames after it do not put the commit in again.
        frame_with(&mut app, &ctx, Vec::new());
        frame_with(&mut app, &ctx, Vec::new());
        assert_eq!(text_of(&app), "한 ", "the commit goes in once");

        let (mut app, ctx) = typing_into("", 0, 0);
        let mut events = vec![ime_commit("日本")];
        events.extend(stroke(K::A, M::NONE, Some("a")));
        frame_with(&mut app, &ctx, events);
        assert_eq!(text_of(&app), "日本a");

        let (mut app, ctx) = typing_into("", 0, 0);
        let mut events = stroke(K::A, M::NONE, Some("a"));
        events.extend([ime_preedit(""), ime_commit("日本")]);
        frame_with(&mut app, &ctx, events);
        assert_eq!(text_of(&app), "a日本");
        let ed = app.doc.editor.as_mut().unwrap();
        assert!(ed.undo());
        assert_eq!(ed.text(), "a", "the commit is an undo step of its own");
    }

    /// 8.21: On a Mac, Option types characters: Option+A, bound to nothing,
    /// types `å`; Option+T still runs the task list and types no `†`.
    /// Elsewhere Alt+A types nothing.
    #[test]
    fn on_a_mac_option_types_its_character_unless_bound() {
        let (mut app, ctx) = typing_into("", 0, 0);
        app.mac = true;
        frame_with(&mut app, &ctx, stroke(K::A, M::ALT, Some("å")));
        assert_eq!(text_of(&app), "å");

        let (mut app, ctx) = typing_into("a\nb", 0, 3);
        app.mac = true;
        frame_with(&mut app, &ctx, stroke(K::T, M::ALT, Some("†")));
        assert_eq!(text_of(&app), by_name("a\nb", 0, 3, "list.task"));

        let (mut app, ctx) = typing_into("", 0, 0);
        app.mac = false;
        frame_with(&mut app, &ctx, stroke(K::A, M::ALT, Some("å")));
        assert_eq!(text_of(&app), "", "Alt+A types nothing but on a Mac");
    }

    /// 8.22: Alt+0165 on the numpad with `word` selected types `¥` over it
    /// and strikes nothing through. The events are in the order ito-keys
    /// reads as a numpad code: the digits pressed with Alt, Alt let go, the
    /// last digit let go, then the character.
    #[test]
    fn an_alt_numpad_code_types_its_character_and_runs_nothing() {
        let (mut app, ctx) = typing_into("word", 0, 4);
        let mut events = vec![Event::ModifiersChanged(M::ALT)];
        events.extend([K::Num0, K::Num1, K::Num6, K::Num5].map(|k| key(k, M::ALT, true)));
        events.extend([Event::ModifiersChanged(M::NONE), key(K::Num5, M::NONE, false), Event::Text("¥".to_owned())]);
        frame_with(&mut app, &ctx, events);
        assert_eq!(text_of(&app), "¥");
    }

    /// 11.10: Alt+W, Alt+Shift+W, Alt+N, Alt+P and Alt+U, as Windows sends
    /// them (the key, then its letter), expand and shrink the selection and
    /// go to the next, the previous and the parent heading as the palette
    /// runs them by name, and type no letter; the palette and the key help
    /// show them, and none is one of weft's own keys or bound twice.
    #[test]
    fn the_structure_alt_keys_select_and_move_and_type_nothing() {
        const DOC: &str = "# A\n\nintro\n\n## B\n\nsome words here\n\n## C\n\nc\n";
        let at = DOC.find("words").unwrap() + 2;
        let (alt, alt_shift) = (M::ALT, M::ALT | M::SHIFT);
        // (run by name first; the key egui reports, what is held, the letter
        // Windows sends; the command; its key in words)
        let grown: &[&str] = &["select.expand", "select.expand"];
        let cases = [
            (&[][..], K::W, alt, "w", "select.expand", "Alt+W"),
            (grown, K::W, alt_shift, "W", "select.shrink", "Alt+Shift+W"),
            (&[], K::N, alt, "n", "cursor.next_heading", "Alt+N"),
            (&[], K::P, alt, "p", "cursor.prev_heading", "Alt+P"),
            (&[], K::U, alt, "u", "cursor.parent_heading", "Alt+U"),
        ];
        for (first, k, mods, typed, name, words) in cases {
            let (mut app, ctx) = typing_into(DOC, at, at);
            let mut want = engine::Editor::new(DOC);
            want.set_caret(at, false);
            for f in first {
                assert_eq!(commands::edit(app.doc.editor.as_mut().unwrap(), f, 0, 20), Some(true));
                assert_eq!(commands::edit(&mut want, f, 0, 20), Some(true));
            }
            assert_eq!(commands::edit(&mut want, name, 0, 20), Some(true), "{name} has something to do");
            frame_with(&mut app, &ctx, stroke(k, mods, Some(typed)));
            let ed = app.doc.editor.as_ref().unwrap();
            assert_eq!(ed.text(), DOC, "{words} types no {typed:?}");
            assert_eq!(ed.selection(), want.selection(), "{words} runs {name}");
            shown_and_bound_once(&app, name, words, k, mods);
        }
    }

    /// 11.12: With the caret at the start of a heading's line that begins
    /// with a quote marker, a list marker or a space (ito#181), Delete the
    /// heading line acts on that heading, its marker going with it, and the
    /// heading above stays; one undo brings it back.
    #[test]
    fn a_heading_after_a_marker_is_deleted_from_the_start_of_its_line() {
        let ctx = egui::Context::default();
        for line in ["> ## Q", "- ## L", " ## C"] {
            let text = format!("# P\n\nbody\n\n{line}\n\nq\n");
            assert_eq!(text.find(line), Some(11));
            let mut app = App::for_tests();
            let mut ed = engine::Editor::new(&text);
            ed.set_caret(11, false);
            app.doc.editor = Some(ed);
            app.run_key(&ctx, keymap::Cmd::Run("heading.delete"));
            assert_eq!(text_of(&app), "# P\n\nbody\n\nq\n", "{line}");
            assert!(app.said.is_empty(), "{line}: {:?}", app.said);
            let ed = app.doc.editor.as_mut().unwrap();
            assert!(ed.undo());
            assert_eq!(ed.text(), text, "{line}: one undo");
        }
    }
}
