//! What the settings screen reads and writes (ito's `docs/common-spec.md`):
//! common.toml (the language, the scale, the theme and the clock, shared by
//! every uchmk app), the person's themes, weft's keymap.toml, and the
//! `[editor]` (the Editing page), `[notify]` (the Notifications page) and
//! `[tabs]` (the tabs' rows of General) tables of weft's config.toml (the rest of config.toml stays a file). The
//! files are watched every 2 s and written
//! on threads of their own; what was read comes back as a [`Reload`], which
//! [`Prefs::pump`] folds in. The UI thread never touches the disk.
//! (koyomi's prefs.rs, without the mail and the limits.)

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

use ito_common::{common_path, edit_common, themes_dir, write_atomic, Common, CommonChange};
use ito_notify::webhook::Place;
use ito_prefs::{Notify, NotifyCfg, State};
use ito_theme::Theme;

use crate::notify::EVENTS;

/// How often the files are looked at.
const WATCH: Duration = Duration::from_secs(2);

/// A file read again, or a write that failed.
pub enum Reload {
    Common(Result<(Common, Vec<String>), String>),
    Themes(Vec<Theme>, Vec<String>),
    /// keymap.toml's text (`None` when there is none), or why it could not
    /// be read.
    Keymap(Result<Option<String>, String>),
    /// config.toml's `[notify]` with the `secret:` addresses read back, its
    /// `[open]` and `[line_args]` (`$EDITOR`), and what to warn of; or why
    /// the file could not be read. Boxed: far larger than the others.
    Config(Result<Box<Config>, String>),
    Failed(String),
}

/// config.toml in `base`'s `weft` folder.
pub fn config_path(base: &Path) -> PathBuf {
    ito_common::app_dir_in(base, "weft").join("config.toml")
}

/// What weft reads of its config.toml.
pub struct Config {
    pub notify: NotifyCfg,
    pub open: ito_openin::Config,
    /// `[tabs]`: when the bar shows, where a file from outside goes, what
    /// reopens at start.
    pub tabs: ito_tabs::Settings,
    pub warnings: Vec<String>,
    /// `[editor] tab_width` and `tab_char`, and what the Editing page warns of.
    pub tab_width: usize,
    pub tab_char: bool,
    pub editor_warn: Vec<String>,
}

/// The tab widths the Editing page offers.
pub const TAB_WIDTHS: [usize; 3] = [2, 4, 8];

#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct EditorFile {
    editor: EditorTable,
}

#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct EditorTable {
    tab_width: Option<i64>,
}

/// `[editor] tab_width` of the config.toml `text` (none: no file), with what
/// to warn of. A width the page does not offer is said, and the default
/// stands in for it.
pub fn read_editor(text: Option<&str>) -> (usize, Vec<String>) {
    let default = crate::engine::TAB_WIDTH;
    let mut warnings = Vec::new();
    let width = match text.map(toml::from_str::<EditorFile>) {
        Some(Ok(f)) => match f.editor.tab_width {
            Some(n) if TAB_WIDTHS.iter().any(|&w| w as i64 == n) => n as usize,
            Some(n) => {
                warnings.push(format!("config.toml: [editor] tab_width = {n} is not 2, 4 or 8; {default} is used"));
                default
            }
            None => default,
        },
        Some(Err(e)) => {
            warnings.push(format!("config.toml: {}", e.message()));
            default
        }
        None => default,
    };
    (width, warnings)
}

/// `[editor] tab_char` alone, read apart from `tab_width` so that a width
/// that is not a number does not hide it.
#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct TabCharFile {
    editor: TabCharTable,
}

#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct TabCharTable {
    tab_char: Option<toml::Value>,
}

/// `[editor] tab_char` of the config.toml `text` (none: no file): whether Tab
/// puts in a tab character instead of spaces, with what to warn of. A value
/// that is not true or false is said, and false stands in for it; a file
/// that does not read is said once, by `read_editor`.
pub fn read_tab_char(text: Option<&str>) -> (bool, Vec<String>) {
    match text.map(toml::from_str::<TabCharFile>) {
        Some(Ok(f)) => match f.editor.tab_char {
            Some(toml::Value::Boolean(on)) => (on, Vec::new()),
            Some(v) => (false, vec![format!("config.toml: [editor] tab_char = {v} is not true or false; false is used")]),
            None => (false, Vec::new()),
        },
        Some(Err(_)) | None => (false, Vec::new()),
    }
}

/// config.toml's `[tabs]` from its `text` (none: the defaults), and what to
/// warn of when it does not read (the defaults then).
pub fn read_tabs(text: Option<&str>) -> (ito_tabs::Settings, Vec<String>) {
    let Some(text) = text else { return Default::default() };
    // The file's own syntax error is said by the `[notify]` reading.
    let Ok(table) = toml::from_str::<toml::Table>(text) else { return Default::default() };
    match table.get("tabs") {
        None => Default::default(),
        Some(v) => match v.clone().try_into::<ito_tabs::Settings>() {
            Ok(s) => (s, Vec::new()),
            Err(e) => (ito_tabs::Settings::default(), vec![format!("config.toml [tabs]: {}", e.message())]),
        },
    }
}

/// `[tabs]` of the config.toml in `base`, read now: what `main` needs before
/// the window opens (whether to hand the files to a running window, whether
/// to reopen the last tabs). Missing or unreadable: the defaults.
pub fn tabs_now(base: Option<&Path>) -> ito_tabs::Settings {
    let text = base.and_then(|b| std::fs::read_to_string(config_path(b)).ok());
    read_tabs(text.as_deref()).0
}

/// The config.toml at `path` (none: the defaults): `[notify]` as the page
/// shows it, the `secret:` addresses read back from the credential store,
/// and `[open]` / `[line_args]`. Blocks on the store: call it off the UI
/// thread.
fn read_config(path: &Path) -> Result<Box<Config>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => Some(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let (mut notify, mut warnings) = crate::notify::read(text.as_deref());
    warnings.extend(notify.reveal("weft"));
    let (open, open_warn) = ito_openin::Config::read(text.as_deref());
    // The file's own syntax error is said once, by the `[notify]` reading.
    let (tabs, tabs_warn) = read_tabs(text.as_deref());
    for w in open_warn.into_iter().chain(tabs_warn) {
        if !warnings.contains(&w) {
            warnings.push(w);
        }
    }
    let (tab_width, mut editor_warn) = read_editor(text.as_deref());
    let (tab_char, char_warn) = read_tab_char(text.as_deref());
    editor_warn.extend(char_warn);
    Ok(Box::new(Config { notify, open, tabs, warnings, tab_width, tab_char, editor_warn }))
}

/// One lock for every writer of config.toml from the window.
static CONFIG_ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Set `edits` (keys of `table`, `None` for the top of the file, already
/// TOML) and `webhooks` in the config.toml at `path` (none yet is an empty
/// one), written only when that changes it; the rest of the file keeps its
/// lines and comments. Call it off the UI thread.
pub fn write_config(path: &Path, table: Option<&str>, edits: &[(&str, String)], webhooks: Option<&[Place]>) -> Result<(), String> {
    let at = |e: String| format!("{}: {e}", path.display());
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(at(e.to_string())),
    };
    let mut new = text.clone();
    for (key, value) in edits {
        new = ito_common::set_key(&new, table, key, value).map_err(at)?;
    }
    if let Some(places) = webhooks {
        new = ito_prefs::write_webhooks(&new, places, true).map_err(at)?;
    }
    if new != text {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| at(e.to_string()))?;
        }
        write_atomic(path, &new).map_err(at)?;
    }
    Ok(())
}

/// keymap.toml in `base`'s `weft` folder.
pub fn keymap_path(base: &Path) -> PathBuf {
    ito_common::app_dir_in(base, "weft").join(crate::keymap::FILE)
}

/// The text of the file at `path`; `None` when there is none.
fn read_keymap(path: &Path) -> Result<Option<String>, String> {
    match std::fs::read_to_string(path) {
        Ok(t) => Ok(Some(t)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// Keys set fast one after another: each reads what the one before wrote,
/// and their keymaps are sent back in the same order.
static KEYMAP_ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Make `edits` in the keymap.toml at `path` (none yet is an empty one),
/// written only when that changes it. Call it off the UI thread.
pub fn write_keymap(path: &Path, edits: &[ito_keymap::Edit]) -> Result<(), String> {
    let at = |e: String| format!("{}: {e}", path.display());
    let text = read_keymap(path)?.unwrap_or_default();
    let new = ito_keymap::edit(&text, &crate::keymap::SPEC, edits, crate::keymap::NOOP).map_err(at)?;
    if new != text {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| at(e.to_string()))?;
        }
        write_atomic(path, &new).map_err(at)?;
    }
    Ok(())
}

/// `<name>.bak-<stamp>` next to `p`: where a file moved aside goes.
fn aside(p: &Path, stamp: u64) -> PathBuf {
    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    p.with_file_name(format!("{name}.bak-{stamp}"))
}

/// Seconds since the epoch, for the names of the files moved aside.
fn stamp() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub struct Prefs {
    /// The `uchmk` folder; `None` when the OS has no config directory.
    pub base: Option<PathBuf>,
    pub common: Common,
    /// The built-in themes, then the person's.
    pub themes: Vec<Theme>,
    pub common_warn: Vec<String>,
    pub theme_warn: Vec<String>,
    /// The theme common.toml names but which is not there.
    pub theme_err: Option<String>,
    /// The screen's page, search and focus.
    pub state: State,
    /// The last write that failed, for the status bar.
    pub failed: Option<String>,
    /// keymap.toml's text, `None` when there is none.
    pub keymap: Option<String>,
    /// `Some(None)` when keymap.toml has been read again (the keys are to be
    /// made again from [`Prefs::keymap`]), `Some(Some(why))` when it could
    /// not be (to be said; the keys stay).
    pub keymap_new: Option<Option<String>>,
    /// The Keys page's search and the keys it is taking.
    pub keys_edit: ito_prefs::KeysEdit,
    /// `[notify]` of config.toml as the Notifications page shows it (the
    /// `secret:` addresses read back), and what it warns of.
    pub notify: NotifyCfg,
    pub notify_warn: Vec<String>,
    /// `[open]` and `[line_args]` of config.toml: the editor `file.open_in_editor` starts.
    pub open: ito_openin::Config,
    /// `[tabs]` of config.toml.
    pub tabs: ito_tabs::Settings,
    /// The Notifications page's fields while they are typed in.
    pub notify_drafts: ito_prefs::Drafts,
    /// `[editor] tab_width` of config.toml: the spaces Tab puts in outside
    /// tables and lists. What the Editing page warns of beside it.
    pub tab_width: usize,
    /// `[editor] tab_char`: Tab puts in a tab character instead of spaces.
    pub tab_char: bool,
    pub editor_warn: Vec<String>,
    tx: Sender<Reload>,
    rx: Receiver<Reload>,
    wake: Option<egui::Context>,
    /// The scale the keys asked for, until common.toml has it: an older
    /// read coming back meanwhile does not undo it.
    scale_wanted: Option<f32>,
    /// The thread that writes the scale, the newest one of a burst.
    scale_tx: Option<Sender<f32>>,
    /// The OS's light or dark, for `system` (egui's lags a switch on Windows).
    os_light: ito_common::OsLight,
}

impl Prefs {
    /// Nothing read: the defaults and the built-in themes (the tests).
    pub fn empty() -> Self {
        let (tx, rx) = channel();
        Self {
            base: None,
            common: Common::default(),
            themes: ito_theme::builtin(),
            common_warn: Vec::new(),
            theme_warn: Vec::new(),
            theme_err: None,
            state: State::default(),
            failed: None,
            keymap: None,
            keymap_new: None,
            keys_edit: ito_prefs::KeysEdit::default(),
            notify: crate::notify::read(None).0,
            notify_warn: Vec::new(),
            open: ito_openin::Config::default(),
            tabs: ito_tabs::Settings::default(),
            notify_drafts: ito_prefs::Drafts::new(),
            tab_width: crate::engine::TAB_WIDTH,
            tab_char: false,
            editor_warn: Vec::new(),
            tx,
            rx,
            wake: None,
            scale_wanted: None,
            scale_tx: None,
            os_light: ito_common::OsLight::default(),
        }
    }

    /// common.toml, the themes and keymap.toml in `base`. config.toml's
    /// `[notify]` comes a moment later (the credential store is slow): a
    /// thread reads it and [`Prefs::pump`] takes it in.
    pub fn load(base: Option<PathBuf>) -> Self {
        let mut p = Self::empty();
        if let Some(b) = &base {
            let (tx, path) = (p.tx.clone(), config_path(b));
            std::thread::spawn(move || {
                let _ = tx.send(Reload::Config(read_config(&path)));
            });
            match Common::read(b) {
                Ok((c, w)) => (p.common, p.common_warn) = (c, w),
                Err(e) => p.common_warn = vec![e],
            }
            let (user, warn) = ito_theme::load_dir(&themes_dir(b));
            p.themes.extend(user);
            p.theme_warn = warn;
            match read_keymap(&keymap_path(b)) {
                Ok(t) => p.keymap = t,
                Err(e) => p.keymap_new = Some(Some(e)),
            }
        }
        p.base = base;
        p
    }

    /// Look at the files and the OS's light or dark every 2 s from now on,
    /// and wake `ctx` when one has been read again.
    pub fn watch(&mut self, ctx: &egui::Context) {
        self.wake = Some(ctx.clone());
        let os_ctx = ctx.clone();
        self.os_light = ito_common::OsLight::watch(WATCH, move || os_ctx.request_repaint());
        let Some(base) = self.base.clone() else { return };
        let (common, themes, keymap, config) = (common_path(&base), themes_dir(&base), keymap_path(&base), config_path(&base));
        let mut paths = vec![common.clone(), themes.clone(), keymap.clone(), config.clone()];
        // A theme file edited in place; one added or taken away changes the
        // folder's time.
        if let Ok(entries) = std::fs::read_dir(&themes) {
            paths.extend(entries.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "toml")));
        }
        let (tx, ctx) = (self.tx.clone(), ctx.clone());
        ito_common::watch(paths, WATCH, move |changed| {
            let mut sent = true;
            if changed.contains(&common) {
                sent &= tx.send(Reload::Common(Common::read(&base))).is_ok();
            }
            if changed.iter().any(|p| p.starts_with(&themes)) {
                let (user, warn) = ito_theme::load_dir(&themes);
                let all = ito_theme::builtin().into_iter().chain(user).collect();
                sent &= tx.send(Reload::Themes(all, warn)).is_ok();
            }
            if changed.contains(&keymap) {
                sent &= tx.send(Reload::Keymap(read_keymap(&keymap))).is_ok();
            }
            if changed.contains(&config) {
                sent &= tx.send(Reload::Config(read_config(&config))).is_ok();
            }
            ctx.request_repaint();
            sent
        });
    }

    /// Fold in what the threads have read since the last frame.
    pub fn pump(&mut self) {
        while let Ok(r) = self.rx.try_recv() {
            match r {
                Reload::Common(Ok((mut c, w))) => {
                    if let Some(s) = self.scale_wanted {
                        if c.scale == Some(s) {
                            self.scale_wanted = None;
                        } else {
                            c.scale = Some(s);
                        }
                    }
                    (self.common, self.common_warn) = (c, w);
                }
                Reload::Common(Err(e)) => self.common_warn = vec![e],
                Reload::Themes(t, w) => (self.themes, self.theme_warn) = (t, w),
                // Unreadable (being written, say): the keys stay as they are.
                Reload::Keymap(Err(e)) => self.keymap_new = Some(Some(e)),
                Reload::Keymap(Ok(t)) => {
                    self.keymap = t;
                    self.keymap_new = Some(None);
                }
                Reload::Config(Ok(c)) => {
                    let c = *c;
                    (self.notify, self.open, self.tabs, self.notify_warn) = (c.notify, c.open, c.tabs, c.warnings);
                    (self.tab_width, self.tab_char, self.editor_warn) = (c.tab_width, c.tab_char, c.editor_warn);
                }
                // Unreadable (being written, say): the pages keep what they
                // had, and say so.
                Reload::Config(Err(e)) => {
                    self.editor_warn = vec![e.clone()];
                    self.notify_warn = vec![e];
                }
                Reload::Failed(e) => {
                    self.scale_wanted = None;
                    self.failed = Some(e);
                }
            }
        }
    }

    fn send_back(tx: &Sender<Reload>, wake: &Option<egui::Context>, r: Reload) {
        let _ = tx.send(r);
        if let Some(ctx) = wake {
            ctx.request_repaint();
        }
    }

    /// Write `changes` into common.toml on a thread, then read it again.
    pub fn common_change(&self, changes: Vec<CommonChange>, no_folder: &str) {
        if changes.is_empty() {
            return;
        }
        let (tx, wake) = (self.tx.clone(), self.wake.clone());
        let Some(base) = self.base.clone() else {
            Self::send_back(&tx, &wake, Reload::Failed(no_folder.into()));
            return;
        };
        std::thread::spawn(move || {
            let done = edit_common(&base, |text| changes.iter().try_fold(text.to_owned(), |t, c| c.apply(&t)));
            let r = match done {
                Ok(()) => Reload::Common(Common::read(&base)),
                Err(e) => Reload::Failed(e),
            };
            Self::send_back(&tx, &wake, r);
        });
    }

    /// The scale at once, and into common.toml on a thread of its own that
    /// writes only the newest of a burst of keys (kura's way).
    pub fn set_scale(&mut self, s: f32, no_folder: &str) {
        self.common.scale = Some(s);
        let Some(base) = self.base.clone() else {
            Self::send_back(&self.tx, &self.wake, Reload::Failed(no_folder.into()));
            return;
        };
        self.scale_wanted = Some(s);
        let tx = self.scale_tx.get_or_insert_with(|| {
            let (tx, rx) = channel::<f32>();
            let (back, wake) = (self.tx.clone(), self.wake.clone());
            std::thread::spawn(move || {
                while let Ok(mut s) = rx.recv() {
                    while let Ok(newer) = rx.try_recv() {
                        s = newer;
                    }
                    let r = match edit_common(&base, |t| ito_common::scale_change(s).apply(t)) {
                        Ok(()) => Reload::Common(Common::read(&base)),
                        Err(e) => Reload::Failed(e),
                    };
                    Self::send_back(&back, &wake, r);
                }
            });
            tx
        });
        let _ = tx.send(s);
    }

    /// Make the Keys page's `edits` in keymap.toml on a thread (only what
    /// differs from the built-in keys is written), then read it again.
    pub fn keymap_edit(&self, edits: Vec<ito_keymap::Edit>, no_folder: &str) {
        if edits.is_empty() {
            return;
        }
        let (tx, wake) = (self.tx.clone(), self.wake.clone());
        let Some(base) = self.base.clone() else {
            Self::send_back(&tx, &wake, Reload::Failed(no_folder.into()));
            return;
        };
        std::thread::spawn(move || {
            let _one = KEYMAP_ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
            let path = keymap_path(&base);
            let r = match write_keymap(&path, &edits) {
                Ok(()) => Reload::Keymap(read_keymap(&path)),
                Err(e) => Reload::Failed(e),
            };
            Self::send_back(&tx, &wake, r);
        });
    }

    /// Write the Notifications page's `page` into config.toml's `[notify]`
    /// on a thread (only the keys that differ from the file; the webhook
    /// addresses go to the credential store while `hide_webhooks` is on),
    /// then read it again.
    pub fn notify_edit(&self, page: Notify, no_folder: &str) {
        if page == self.notify.page(&EVENTS) {
            return;
        }
        let (tx, wake) = (self.tx.clone(), self.wake.clone());
        let Some(base) = self.base.clone() else {
            Self::send_back(&tx, &wake, Reload::Failed(no_folder.into()));
            return;
        };
        let now = self.notify.clone();
        std::thread::spawn(move || {
            let _one = CONFIG_ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
            let path = config_path(&base);
            let edits = now.edits(&EVENTS, &page);
            let r = match now.webhooks_to_write("weft", &EVENTS, &page) {
                Ok(webhooks) => match write_config(&path, Some("notify"), &edits, webhooks.as_deref()) {
                    Ok(()) => Reload::Config(read_config(&path)),
                    Err(e) => Reload::Failed(e),
                },
                Err(e) => Reload::Failed(format!("{}: {e}", path.display())),
            };
            Self::send_back(&tx, &wake, r);
        });
    }

    /// The Editing page's tab width at once, and into config.toml's
    /// `[editor]` on a thread (the rest of the file stays as it is), then
    /// the file read again.
    pub fn tab_width_edit(&mut self, width: usize, no_folder: &str) {
        if width == self.tab_width {
            return;
        }
        self.tab_width = width;
        self.editor_write("tab_width", width.to_string(), no_folder);
    }

    /// The Editing page's "tab character" switch at once, and into
    /// config.toml's `[editor]` as `tab_width` goes.
    pub fn tab_char_edit(&mut self, on: bool, no_folder: &str) {
        if on == self.tab_char {
            return;
        }
        self.tab_char = on;
        self.editor_write("tab_char", on.to_string(), no_folder);
    }

    /// Set `key` of config.toml's `[editor]` to `value` (already TOML) on a
    /// thread, then read the file again.
    fn editor_write(&self, key: &'static str, value: String, no_folder: &str) {
        let (tx, wake) = (self.tx.clone(), self.wake.clone());
        let Some(base) = self.base.clone() else {
            Self::send_back(&tx, &wake, Reload::Failed(no_folder.into()));
            return;
        };
        std::thread::spawn(move || {
            let _one = CONFIG_ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
            let path = config_path(&base);
            let r = match write_config(&path, Some("editor"), &[(key, value)], None) {
                Ok(()) => Reload::Config(read_config(&path)),
                Err(e) => Reload::Failed(e),
            };
            Self::send_back(&tx, &wake, r);
        });
    }

    /// The tabs' rows of General: `s` at once, and into config.toml's
    /// `[tabs]` on a thread (only the keys that differ from the file), then
    /// read again.
    pub fn tabs_edit(&mut self, s: ito_tabs::Settings, no_folder: &str) {
        if s == self.tabs {
            return;
        }
        self.tabs = s;
        let (tx, wake) = (self.tx.clone(), self.wake.clone());
        let Some(base) = self.base.clone() else {
            Self::send_back(&tx, &wake, Reload::Failed(no_folder.into()));
            return;
        };
        std::thread::spawn(move || {
            let _one = CONFIG_ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
            let path = config_path(&base);
            let r = match write_config(&path, Some("tabs"), &tabs_edits(&s), None) {
                Ok(()) => Reload::Config(read_config(&path)),
                Err(e) => Reload::Failed(e),
            };
            Self::send_back(&tx, &wake, r);
        });
    }

    /// Open a settings file of the base (`common_path`, `config_path`), made
    /// empty first when there is none, with what the OS opens it with, on a
    /// thread. An empty file reads as the defaults, as no file does.
    pub fn open_file(&self, file: fn(&Path) -> PathBuf, no_folder: &str) {
        let (tx, wake) = (self.tx.clone(), self.wake.clone());
        let Some(base) = self.base.clone() else {
            Self::send_back(&tx, &wake, Reload::Failed(no_folder.into()));
            return;
        };
        std::thread::spawn(move || {
            let path = file(&base);
            let made = if path.exists() {
                Ok(())
            } else {
                path.parent()
                    .map_or(Ok(()), |d| std::fs::create_dir_all(d).map_err(|e| e.to_string()))
                    .and_then(|()| write_atomic(&path, ""))
            };
            if let Err(e) = made.and_then(|()| open_path(&path)) {
                Self::send_back(&tx, &wake, Reload::Failed(e));
            }
        });
    }

    /// Open a folder in the system's file manager, on a thread.
    pub fn open_dir(&self, dir: PathBuf) {
        let (tx, wake) = (self.tx.clone(), self.wake.clone());
        std::thread::spawn(move || {
            if let Err(e) = open_path(&dir) {
                Self::send_back(&tx, &wake, Reload::Failed(e));
            }
        });
    }

    /// The shared settings back to how weft starts: common.toml and
    /// keymap.toml are moved aside (`<name>.bak-<time>`), on a thread. The
    /// watcher then reads the missing files as defaults. weft's config.toml
    /// is not touched.
    pub fn reset(&self, no_folder: &str) {
        let (tx, wake) = (self.tx.clone(), self.wake.clone());
        let Some(base) = self.base.clone() else {
            Self::send_back(&tx, &wake, Reload::Failed(no_folder.into()));
            return;
        };
        std::thread::spawn(move || {
            let stamp = stamp();
            let mut failed = Vec::new();
            for path in [common_path(&base), keymap_path(&base)] {
                if path.exists() {
                    if let Err(e) = std::fs::rename(&path, aside(&path, stamp)) {
                        failed.push(format!("{}: {e}", path.display()));
                    }
                }
            }
            let r = match failed.first() {
                None => Reload::Common(Common::read(&base)),
                Some(e) => Reload::Failed(e.clone()),
            };
            Self::send_back(&tx, &wake, r);
        });
    }

    /// The theme common.toml picks, in force from this frame on. Called
    /// every frame (the OS may have gone light or dark).
    pub fn apply_theme(&mut self, ctx: &egui::Context) {
        let os_light = self.os_light.light_or(ctx.system_theme() == Some(egui::Theme::Light));
        let (choice, dark, light) = self.common.theme_choice(crate::settings::THEMES);
        let colors = match ito_theme::pick(&self.themes, &choice, &dark, &light, os_light) {
            Ok(t) => {
                self.theme_err = None;
                t.colors
            }
            Err(e) => {
                self.theme_err = Some(e);
                self.themes[0].colors
            }
        };
        // `high_contrast = true` draws every theme in its stronger colours, as the Web page does.
        let colors = if self.common.high_contrast_or_off() { colors.high_contrast() } else { colors };
        if colors != ito_theme::colors() {
            ito_theme::set(colors);
            ctx.set_visuals(colors.visuals());
        }
    }
}

fn open_path(path: &Path) -> Result<(), String> {
    ito_openin::open_default(path)
}

/// The keys of `[tabs]` the settings screen sets, as TOML values.
fn tabs_edits(s: &ito_tabs::Settings) -> Vec<(&'static str, String)> {
    let v = |x: Result<toml::Value, toml::ser::Error>| x.map(|v| v.to_string()).unwrap_or_default();
    vec![
        ("show_bar", v(toml::Value::try_from(s.show_bar))),
        ("open_from_outside", v(toml::Value::try_from(s.open_from_outside))),
        ("reopen", v(toml::Value::try_from(s.reopen))),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use ito_keymap::Edit;
    use ito_keys::Key;

    fn on(t: &str) -> Vec<Key> {
        t.split_whitespace().map(|k| Key::parse(k).unwrap()).collect()
    }

    /// 3.4: Only what is changed is written; put back, nothing of it stays.
    #[test]
    fn the_keymap_gets_only_the_changes() {
        let dir = std::env::temp_dir().join(format!("weft-keymap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = keymap_path(&dir);
        let add = Edit::Add { layer: "app".into(), on: on("<C-S-h>"), run: "help".into(), desc: "Show the key help".into() };
        let reset = Edit::Reset { layer: "app".into(), run: "help".into() };
        // Added then put back in one go: no file is made.
        write_keymap(&path, &[add.clone(), reset.clone()]).unwrap();
        assert!(!path.exists());
        write_keymap(&path, &[add]).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("<C-S-h>") && text.contains("prepend_keymap") && !text.contains("quit"), "{text}");
        // A built-in key taken away is a `none` in front of it.
        write_keymap(&path, &[Edit::Remove { layer: "app".into(), on: on("<C-q>"), run: "quit".into() }]).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(r#"run = "none""#), "{text}");
        let (k, warnings) = crate::keymap::Keys::load(Some(("keymap.toml", &text)));
        assert!(warnings.is_empty(), "{warnings:?}");
        let help: Vec<_> = k.layers()[0].1.iter().filter(|b| b.raw == "help").map(|b| b.on.clone()).collect();
        assert!(help.contains(&on("<C-S-h>")), "{help:?}");
        write_keymap(&path, &[reset]).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("help") && !text.contains("<C-S-h>"), "{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The Notifications page's change lands in `[notify]` and nothing else
    /// of the file moves.
    #[test]
    fn the_config_keeps_its_other_lines() {
        let dir = std::env::temp_dir().join(format!("weft-config-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = config_path(&dir);
        write_config(&path, Some("notify"), &[("focus_mode", "false".into())], None).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("[notify]") && text.contains("focus_mode = false"), "{text}");
        let with_comment = format!("# mine\n{text}");
        std::fs::write(&path, &with_comment).unwrap();
        write_config(&path, Some("notify"), &[("focus_mode", "false".into())], None).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), with_comment);
        let c = read_config(&path).unwrap();
        assert!(c.warnings.is_empty() && !c.notify.focus_mode(), "{:?}", c.warnings);
        std::fs::write(&path, format!("{with_comment}[open]\neditor = \"hx {{path}}\"\n")).unwrap();
        let c = read_config(&path).unwrap();
        assert_eq!(c.open.open.editor, "hx {path}", "{:?}", c.warnings);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The Editing page's tab width lands in `[editor] tab_width` and is
    /// read back; the file's other lines stay, and a width the page does not
    /// offer is said and stands at 4. (3.9 checks the page itself.)
    #[test]
    fn the_tab_width_is_kept_in_the_editor_table() {
        let dir = std::env::temp_dir().join(format!("weft-tabwidth-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = config_path(&dir);
        assert_eq!(read_editor(None), (4, vec![]));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "# mine\n[notify]\nfocus_mode = false\n").unwrap();
        for width in TAB_WIDTHS {
            write_config(&path, Some("editor"), &[("tab_width", width.to_string())], None).unwrap();
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(text.starts_with("# mine\n[notify]\nfocus_mode = false\n"), "{text}");
            assert!(text.contains("[editor]") && text.contains(&format!("tab_width = {width}")), "{text}");
            assert_eq!(read_editor(Some(&text)), (width, vec![]));
            let c = read_config(&path).unwrap();
            assert!(c.editor_warn.is_empty() && c.tab_width == width, "{:?}", c.editor_warn);
        }
        let (width, warnings) = read_editor(Some("[editor]\ntab_width = 3\n"));
        assert_eq!(width, 4);
        assert!(warnings.len() == 1 && warnings[0].contains("tab_width = 3"), "{warnings:?}");
        let (width, warnings) = read_editor(Some("[editor]\ntab_width = \"wide\"\n"));
        assert!(width == 4 && warnings.len() == 1, "{warnings:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The Editing page's "tab character" switch lands in `[editor] tab_char`
    /// beside the width and is read back; the other lines stay, a value that
    /// is not true or false is said and stands at false, and a width that
    /// does not read does not hide it. (3.10 checks the page itself.)
    #[test]
    fn the_tab_character_is_kept_beside_the_tab_width() {
        let dir = std::env::temp_dir().join(format!("weft-tabchar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = config_path(&dir);
        assert_eq!(read_tab_char(None), (false, vec![]));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "# mine\n[editor]\ntab_width = 8\n").unwrap();
        for on in [true, false, true] {
            write_config(&path, Some("editor"), &[("tab_char", on.to_string())], None).unwrap();
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(text.starts_with("# mine\n[editor]\ntab_width = 8\n"), "{text}");
            assert!(text.contains(&format!("tab_char = {on}")), "{text}");
            assert_eq!(read_tab_char(Some(&text)), (on, vec![]));
            let c = read_config(&path).unwrap();
            assert!(c.editor_warn.is_empty() && c.tab_char == on && c.tab_width == 8, "{:?}", c.editor_warn);
        }
        let (on, warnings) = read_tab_char(Some("[editor]\ntab_char = \"yes\"\n"));
        assert!(!on && warnings.len() == 1 && warnings[0].contains("tab_char"), "{warnings:?}");
        assert_eq!(read_tab_char(Some("[editor]\ntab_width = \"wide\"\ntab_char = true\n")), (true, vec![]));
        assert_eq!(read_tab_char(Some("[editor\n")), (false, vec![]));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 12.9: The tabs' rows of General land in config.toml's `[tabs]` (not
    /// common.toml), read back as they were set, beside the other tables;
    /// a value that does not read is the defaults and a warning.
    #[test]
    fn the_tab_settings_go_to_the_config_and_back() {
        use ito_tabs::{OpenFromOutside, Reopen, Settings, ShowBar};
        let dir = std::env::temp_dir().join(format!("weft-tabs-config-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = config_path(&dir);
        assert_eq!(tabs_now(Some(&dir)), Settings::default(), "no file: the defaults");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "# mine\n[notify]\nfocus_mode = false\n").unwrap();
        let s = Settings {
            show_bar: ShowBar::Always,
            open_from_outside: OpenFromOutside::NewWindow,
            reopen: Reopen::OnlyGiven,
            ..Settings::default()
        };
        write_config(&path, Some("tabs"), &tabs_edits(&s), None).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("# mine\n[notify]\nfocus_mode = false\n") && text.contains("[tabs]"), "{text}");
        assert!(text.contains(r#"show_bar = "always""#) && text.contains(r#"reopen = "only_given""#), "{text}");
        assert_eq!(tabs_now(Some(&dir)), s);
        let c = read_config(&path).unwrap();
        assert_eq!(c.tabs, s);
        assert!(c.warnings.is_empty(), "{:?}", c.warnings);
        assert!(!common_path(&dir).exists(), "nothing of it is common.toml's");
        // Unchanged: the file is not written again.
        write_config(&path, Some("tabs"), &tabs_edits(&s), None).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
        let (bad, warn) = read_tabs(Some("[tabs]\nshow_bar = \"sometimes\"\n"));
        assert_eq!(bad, Settings::default());
        assert!(warn.len() == 1 && warn[0].starts_with("config.toml [tabs]:"), "{warn:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A PC with no theme in common.toml gets a built-in theme and no
    /// warning under the bell (the warning used to come every frame).
    #[test]
    fn no_theme_in_common_toml_is_a_built_in_one_and_no_warning() {
        let ctx = egui::Context::default();
        let mut p = Prefs::empty();
        p.apply_theme(&ctx);
        assert_eq!(p.theme_err, None);
    }

    #[test]
    fn a_file_moved_aside_keeps_its_name_with_a_stamp() {
        let p = aside(Path::new("C:/uchmk/common.toml"), 42);
        assert_eq!(p.file_name().unwrap().to_string_lossy(), "common.toml.bak-42");
        assert_eq!(p.parent(), Some(Path::new("C:/uchmk")));
    }
}
