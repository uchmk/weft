//! The settings screen (Ctrl+,): `ito-prefs` draws the nav and the cards, and
//! the rows common to every uchmk app (the language, the size, the clock,
//! the window, the theme) write common.toml. weft's own pages are the Editing
//! page (ito's app page: the tab width, `[editor]` of weft's config.toml),
//! the Keys page (each command's keys, written to keymap.toml) and the
//! Notifications page (`[notify]` of weft's config.toml). (koyomi's
//! settings.rs, without the records, the sign-in start and the mail.)

use egui::{vec2, Align2, FontId, Sense, Stroke, StrokeKind, Ui};
use ito_common::{themes_dir, CommonChange};
use ito_prefs::{section, MoreWords, Words};

use crate::App;

// ito's default order (`ito_prefs::default_order`); the Editing page is
// ito's `Page::App`.
pub const GENERAL: usize = 0;
pub const THEME: usize = 1;
pub const EDITOR: usize = 2;
pub const KEYS: usize = 3;
pub const NOTIFY: usize = 4;
pub const ADVANCED: usize = 5;
pub const ABOUT: usize = 6;

/// The keymap's layers and the words that name them on the Keys page.
const LAYERS: [(&str, &str); 2] = [("app", "keys.main"), ("edit", "keys.edit")];

/// What common.toml means when it names no theme: ito's default (`dark`,
/// tsumugi Dark and tsumugi Light, as the common spec says). weft has no
/// themes of its own, and no older setting to carry over.
pub const THEMES: Option<(&str, &str, &str)> = None;

/// weft's own settings file, opened from the foot of the settings' list.
const CONFIG_FILE: &str = "config.toml";

/// The rows on each page, for the search.
fn index(w: &'static Words, more: &'static MoreWords, lang: &str, t: &ito_i18n::Table) -> Vec<(usize, String)> {
    let keys = ito_prefs::keys_index(ito_prefs::KeyWords::of(lang)).into_iter().map(|t| (KEYS, t));
    let layers = LAYERS.iter().map(|(_, k)| (KEYS, t.tr(k)));
    let editor = ["settings.tab", "settings.tab_width", "settings.tab_char"].into_iter().map(|k| (EDITOR, t.tr(k)));
    vec![
        (GENERAL, w.language),
        (GENERAL, w.scale),
        (GENERAL, w.clock),
        (GENERAL, w.show_time),
        (GENERAL, w.time_format),
        (GENERAL, w.show_date),
        (GENERAL, w.date_format),
        (GENERAL, w.weekday),
        (GENERAL, "language"),
        (GENERAL, "scale"),
        (GENERAL, "[clock]"),
        (GENERAL, "[window]"),
        (GENERAL, "[tabs]"),
        (THEME, w.mode),
        (THEME, w.themes),
        (THEME, w.preview),
        (THEME, "theme"),
        (THEME, "dark_theme"),
        (THEME, "light_theme"),
        (EDITOR, "[editor]"),
        (EDITOR, "tab_width"),
        (EDITOR, "tab_char"),
        (EDITOR, CONFIG_FILE),
        (KEYS, "keymap.toml"),
        (KEYS, "prepend_keymap"),
        (NOTIFY, "[notify]"),
        (NOTIFY, CONFIG_FILE),
    ]
    .into_iter()
    .chain(ito_prefs::notify_index(w).into_iter().map(|t| (NOTIFY, t)))
    .chain(keys)
    .chain(layers)
    .chain(editor)
    .chain(ito_prefs::window_index(more).into_iter().map(|t| (GENERAL, t)))
    .chain(ito_prefs::tabs_index(ito_prefs::TabsWords::of(lang)).into_iter().map(|t| (GENERAL, t)))
    .chain(ito_prefs::TABS_KEYS.into_iter().map(|t| (GENERAL, t)))
    .chain(ito_prefs::advanced_index(more).into_iter().map(|t| (ADVANCED, t)))
    .chain(ito_prefs::mono_font_index(more).into_iter().map(|t| (THEME, t)))
    .chain(ito_prefs::about_index(more, w).into_iter().map(|t| (ABOUT, t)))
    .map(|(p, t)| (p, t.to_owned()))
    .collect()
}

pub fn show(ui: &mut Ui, app: &mut App) {
    let w = Words::of(&app.lang);
    let more = MoreWords::of(&app.lang);
    let no_folder = app.table.tr("settings.no_folder").to_owned();
    // While a key is being set, the page takes every key, Esc included.
    let keys = if app.prefs.keys_edit.busy() { ito_prefs::Keys::default() } else { ito_prefs::Keys::standard(ui, false) };
    let table = &app.table;
    let tr = |k: &str| table.tr(k).to_owned();
    let (adv, about) = (ito_prefs::Page::Advanced.words(more), ito_prefs::Page::About.words(more));
    let page_words = [
        (tr("settings.general"), tr("settings.general_lead")),
        (tr("settings.theme"), tr("settings.theme_lead")),
        (tr("settings.editor"), tr("settings.editor_lead")),
        (tr("settings.keys"), tr("settings.keys_lead")),
        (tr("settings.notify"), tr("settings.notify_lead")),
        (adv.0.to_owned(), adv.1.to_owned()),
        (about.0.to_owned(), about.1.to_owned()),
    ];
    let pages: Vec<(&str, &str)> = page_words.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let index = index(w, more, &app.lang, table);
    let index: Vec<(usize, &str)> = index.iter().map(|(p, t)| (*p, t.as_str())).collect();
    let nav = ito_prefs::Nav { pages: &pages, index: &index, words: w, file: "common.toml" };
    let mut asked: Vec<CommonChange> = Vec::new();
    let layers = app.keys.layers().to_vec();
    let kw = ito_prefs::KeyWords::of(&app.lang);
    let own =
        app.prefs.base.as_deref().map_or_else(|| crate::keymap::FILE.to_owned(), |b| crate::prefs::keymap_path(b).display().to_string());
    let mut key_edits = Vec::new();
    // The Notifications page as it is set now, when it was changed; and
    // what "Send a test" asked for.
    let mut notify_new: Option<ito_prefs::Notify> = None;
    let mut test = None;
    // The tab width picked on the Editing page, and the tab character
    // switched on or off.
    let mut tab_new: Option<usize> = None;
    let mut tab_char_new: Option<bool> = None;
    // The `[tabs]` rows, written back to config.toml when they change.
    let tabs_words = ito_prefs::TabsWords::of(&app.lang);
    let mut tabs_now = app.prefs.tabs;
    let prefs = &mut app.prefs;
    let keys_edit = &mut prefs.keys_edit;
    let (notify, notify_warn, drafts) = (&prefs.notify, &prefs.notify_warn, &mut prefs.notify_drafts);
    let (tab_now, tab_char_now, editor_warn) = (prefs.tab_width, prefs.tab_char, &prefs.editor_warn);
    let mut bug = false;
    let mut forget = false;
    let mut advanced = Vec::new();
    let state_dir = ito_common::state_dir("weft");
    let (common, themes, base) = (&prefs.common, &prefs.themes, prefs.base.as_deref());
    // config.toml's button above common.toml's: weft's own settings are a file of their own.
    let extras = ito_prefs::Extras { files: &[CONFIG_FILE], ..Default::default() };
    let out = ito_prefs::show_with(ui, ito_theme::colors(), &mut prefs.state, &nav, keys, extras, |ui, page, l| match page {
        GENERAL => {
            section(ui, l, &tr("settings.display"), |ui| {
                let now = common.language.as_deref().unwrap_or("auto");
                asked.extend(ito_prefs::language_row(ui, l, w, now, ""));
                ito_prefs::sep(ui, l);
                asked.extend(ito_prefs::scale_row(ui, l, w, common.scale_or_one(), ""));
            });
            asked.extend(ito_prefs::clock_card(ui, l, w, &common.clock_or(None)));
            let out = ito_prefs::window_card(ui, l, more, &common.window_or_default());
            asked.extend(out.changes);
            forget |= out.forget;
            section(ui, l, &tr("settings.tabs"), |ui| {
                ito_prefs::tabs_rows_in(ui, l, tabs_words, &mut tabs_now);
            });
        }
        THEME => {
            let (t, d, li) = common.theme_choice(THEMES);
            let note = match base {
                Some(b) => format!("{} {}", tr("settings.theme_note"), themes_dir(b).display()),
                None => tr("settings.no_folder"),
            };
            asked.extend(ito_prefs::theme_page(ui, l, w, themes, (&t, &d, &li), &note, |ui| preview(ui, &tr("settings.sample"))));
            // Read at the next start: the fonts are loaded once, in the background.
            asked.extend(ito_prefs::mono_font_card(ui, l, more, common.mono_font_or_default()));
        }
        EDITOR => {
            for n in editor_warn {
                ito_prefs::status(ui, n, l.c.err);
            }
            let widths = crate::prefs::TAB_WIDTHS.map(|n| (n, n.to_string()));
            let options: Vec<(usize, &str)> = widths.iter().map(|(n, s)| (*n, s.as_str())).collect();
            // The rows may be drawn twice (the search): they only say what was picked.
            section(ui, l, &tr("settings.tab"), |ui| {
                let (label, note) = (tr("settings.tab_width"), tr("settings.tab_width_note"));
                let picked = ito_prefs::keyed_row(ui, l, "tab_width", &label, &note, |ui| {
                    ito_prefs::seg_named(ui, l, "tab_width", &label, tab_now, &options).0
                });
                if picked.is_some() {
                    tab_new = picked;
                }
                ito_prefs::sep(ui, l);
                let (label, note) = (tr("settings.tab_char"), tr("settings.tab_char_note"));
                if ito_prefs::keyed_row(ui, l, "tab_char", &label, &note, |ui| ito_prefs::switch(ui, l, tab_char_now)) {
                    tab_char_new = Some(!tab_char_now);
                }
            });
        }
        KEYS => {
            let layer_name = |n: &str| LAYERS.iter().find(|(id, _)| *id == n).map_or_else(|| n.to_owned(), |(_, k)| tr(k));
            let desc = |d: &str| table.tr_desc(d).to_owned();
            // Keys weft reads before the keymap: bound, they never run.
            let extra = |_: &str, on: &[ito_keys::Key]| -> Vec<String> {
                crate::keymap::Keys::fixed(on).map(|k| tr("keys.fixed").replace("{}", &k.human())).into_iter().collect()
            };
            let table = ito_prefs::KeyTable {
                now: &layers,
                built_in: crate::keymap::Keys::built_in_layers(),
                noop: crate::keymap::NOOP,
                own: &own,
                layer_name: &layer_name,
                tr: &desc,
                extra: &extra,
                unbound: crate::keymap::unbound(),
                sup: false,
            };
            key_edits.extend(ito_prefs::keys_page(ui, l, kw, &table, keys_edit).unwrap_or_default());
        }
        NOTIFY => {
            for n in notify_warn {
                ito_prefs::status(ui, n, l.c.err);
            }
            let mut n = notify.page(&crate::notify::EVENTS);
            let was = n.clone();
            let (failed, failed_sound) = (tr("notify.save_failed"), tr("notify.save_failed_sound"));
            let ends = [ito_prefs::End { name: &failed, color: l.c.err, sound: &failed_sound }];
            let defaults = crate::notify::read(None).0.page(&crate::notify::EVENTS);
            if let Some(what) = ito_prefs::notify_page(ui, l, w, &ends, &mut n, &defaults, drafts) {
                test = Some((what, n.clone()));
            }
            if n != was {
                notify_new = Some(n);
            }
        }
        ADVANCED => advanced.extend(ito_prefs::advanced_card(ui, l, more, state_dir.is_some())),
        ABOUT => {
            let system = ito_bugreport::os_line();
            let license = Some(env!("CARGO_PKG_LICENSE")).filter(|x| !x.is_empty());
            let about =
                ito_prefs::About { version: env!("CARGO_PKG_VERSION"), commit: option_env!("WEFT_COMMIT"), system: &system, license };
            bug |= !ito_prefs::about_page(ui, l, more, w, "weft", &about).is_empty();
        }
        _ => {}
    });
    if bug {
        app.report_bug(ui.ctx());
    }
    if let Some((what, n)) = test {
        crate::notify::test(&n, what, &tr("notify.test_title"), &tr("notify.test_body"));
    }
    if let Some(n) = notify_new {
        app.prefs.notify_edit(n, &no_folder);
    }
    if let Some(n) = tab_new {
        app.prefs.tab_width_edit(n, &no_folder);
    }
    if let Some(on) = tab_char_new {
        app.prefs.tab_char_edit(on, &no_folder);
    }
    app.prefs.tabs_edit(tabs_now, &no_folder);
    if forget {
        if let Some(dir) = &state_dir {
            let dir = dir.clone();
            std::thread::spawn(move || {
                let _ = ito_prefs::forget_place(&dir);
            });
        }
    }
    for a in advanced {
        match a {
            ito_prefs::Advanced::OpenConfigDir => match app.prefs.base.clone() {
                Some(b) => app.prefs.open_dir(b),
                None => app.note = Some(no_folder.clone()),
            },
            ito_prefs::Advanced::OpenStateDir => {
                if let Some(d) = state_dir.clone() {
                    app.prefs.open_dir(d);
                }
            }
            // common.toml and keymap.toml are moved aside on a thread;
            // config.toml stays (the Notifications page has its own ↺).
            ito_prefs::Advanced::Reset => app.prefs.reset(&no_folder),
        }
    }
    if out.close {
        app.settings_open = false;
    }
    match out.open_file {
        Some(0) => app.prefs.open_file(ito_common::common_path, &no_folder),
        Some(_) => app.prefs.open_file(crate::prefs::config_path, &no_folder),
        None => {}
    }
    // Gone to another page: the keys stop being taken for this one.
    if app.prefs.state.page != KEYS {
        app.prefs.keys_edit.capture = None;
        app.prefs.keys_edit.listening = false;
    }
    app.prefs.keymap_edit(key_edits, &no_folder);
    app.prefs.common_change(asked, &no_folder);
}

/// The Theme page's sample: a card with a few Markdown lines in the
/// colours the editor will draw them with.
fn preview(ui: &mut Ui, sample: &str) {
    let c = ito_theme::colors();
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width().min(420.0), 96.0), Sense::hover());
    let p = ui.painter_at(r);
    p.rect_filled(r, 8.0, c.bg);
    p.rect_stroke(r, 8.0, Stroke::new(1.0, c.border), StrokeKind::Inside);
    let inner = r.shrink(12.0);
    let mono = FontId::monospace(12.0);
    p.text(inner.left_top(), Align2::LEFT_TOP, sample, FontId::proportional(12.0), c.dim);
    let lines = [("# 見出し / Heading", c.run), ("- [ ] タスク / task", c.fg), ("**強調** / strong", c.done)];
    for (i, (text, color)) in lines.into_iter().enumerate() {
        p.text(egui::pos2(inner.left(), inner.top() + 22.0 + i as f32 * 18.0), Align2::LEFT_TOP, text, mono.clone(), color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ito_prefs::Page;

    #[test]
    fn pages_follow_ito_default_order() {
        let ours = [GENERAL, THEME, EDITOR, KEYS, NOTIFY, ADVANCED, ABOUT];
        assert!(ours.windows(2).all(|w| w[0] + 1 == w[1]));
        let has = [Page::Theme, Page::App, Page::Keys, Page::Notifications, Page::Advanced];
        assert_eq!(
            ito_prefs::default_order(&has),
            [Page::General, Page::Theme, Page::App, Page::Keys, Page::Notifications, Page::Advanced, Page::About]
        );
    }

    /// The Keys page's words, and weft's own, are found by the search.
    #[test]
    fn the_search_finds_the_keys_page() {
        let (t, errors) = crate::i18n::table("en");
        assert!(errors.is_empty(), "{errors:?}");
        let idx = index(Words::of("en"), MoreWords::of("en"), "en", &t);
        assert!(idx.iter().any(|(p, t)| *p == KEYS && t == "keymap.toml"));
        assert!(idx.iter().any(|(p, t)| *p == KEYS && t == "Everywhere"));
        assert!(idx.iter().any(|(p, t)| *p == GENERAL && t == "[window]"));
        assert!(idx.iter().any(|(p, t)| *p == NOTIFY && t == "config.toml"));
        assert!(idx.iter().any(|(p, t)| *p == EDITOR && t == "tab_width"));
        assert!(idx.iter().any(|(p, t)| *p == EDITOR && t == "Tab width"));
        assert!(idx.iter().any(|(p, t)| *p == EDITOR && t == "tab_char"));
        assert!(idx.iter().any(|(p, t)| *p == EDITOR && t == "Insert a tab character"));
    }

    /// The `[tabs]` rows are on the General page, found by their words and by
    /// config.toml's keys.
    #[test]
    fn the_search_finds_the_tabs_rows() {
        let (t, _) = crate::i18n::table("ja");
        let idx = index(Words::of("ja"), MoreWords::of("ja"), "ja", &t);
        assert!(idx.iter().any(|(p, t)| *p == GENERAL && t == "[tabs]"));
        assert!(idx.iter().any(|(p, t)| *p == GENERAL && t == "tabs.show_bar"));
        let words = ito_prefs::tabs_index(ito_prefs::TabsWords::of("ja"));
        assert!(words.iter().all(|w| idx.iter().any(|(p, t)| *p == GENERAL && t == w)));
    }
}
