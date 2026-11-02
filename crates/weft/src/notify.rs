//! Telling of a save that failed while the window is behind another, the
//! ways `[notify]` in weft's config.toml picks (common-spec, "通知"): the
//! system's notification with weft's icon, a sound, and the webhooks. And
//! `[notify]` as the settings' Notifications page (`ito_prefs::notify_page`)
//! shows it. (fude's notify.rs.)

pub use ito_prefs::NotifyCfg;
use serde::Deserialize;

/// Who the notifications are from.
const APP: ito_notify::toast::App = ito_notify::toast::App { id: "uchmk.weft", name: "weft" };

/// What weft tells of: a file it could not write.
pub const EVENTS: [ito_prefs::Event; 1] =
    [ito_prefs::Event { name: "save_failed", sound_key: "sound_save_failed", sound: "chime", level: ito_notify::webhook::Level::Error }];

#[derive(Deserialize, Default)]
#[serde(default)]
struct File {
    notify: NotifyCfg,
}

/// `[notify]` of the config.toml `text` (none: no file), with what to warn of.
pub fn read(text: Option<&str>) -> (NotifyCfg, Vec<String>) {
    let mut warnings = Vec::new();
    let mut n = match text.map(toml::from_str::<File>) {
        Some(Ok(f)) => f.notify,
        Some(Err(e)) => {
            warnings.push(format!("config.toml: {}", e.message()));
            NotifyCfg::default()
        }
        None => NotifyCfg::default(),
    };
    n.check(&EVENTS, "config.toml", &mut warnings);
    (n, warnings)
}

/// weft's icon at `side` pixels, as the window's.
fn icon(side: u32) -> Vec<u8> {
    crate::icon::pixels(side)
}

/// Tell of a save that failed in every way `n` turns on for it. A click on
/// the system's notification (or its `open` button) brings the window
/// forward; a newer failure's replaces the last.
pub fn tell(n: &NotifyCfg, title: &str, body: &str, open: &str) {
    n.tell_open(&EVENTS, APP, icon, "save_failed", title, body, "save_failed", open);
}

/// "Send a test", as the page sets things now: every way, or only the
/// webhook `what` names.
pub fn test(p: &ito_prefs::Notify, what: ito_prefs::Test, title: &str, body: &str) {
    ito_prefs::test_notify(&EVENTS, p, what, APP, icon, title, body);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_not_known_is_warned_of_and_set_back() {
        let (n, warn) = read(Some(
            "[notify]
system = [\"save_failed\", \"done\"]
sound_save_failed = \"boing\"
",
        ));
        assert_eq!(n.system, Some(vec!["save_failed".to_owned()]), "{warn:?}");
        assert_eq!(n.sounds[0], "chime");
        assert_eq!(warn.len(), 2, "{warn:?}");
        assert!(warn.iter().all(|w| w.starts_with("config.toml: [notify]")), "{warn:?}");
        let (n, warn) = read(None);
        assert!(warn.is_empty() && n == NotifyCfg::new(&EVENTS));
        let (_, warn) = read(Some(
            "[notify
",
        ));
        assert_eq!(warn.len(), 1);
    }

    #[test]
    fn notify_with_no_webhooks_and_the_other_tables_are_fine() {
        let (n, warn) = read(Some(
            "[editor]
wrap = true
[notify]
focus_mode = false
",
        ));
        assert!(warn.is_empty(), "{warn:?}");
        assert!(!n.focus_mode() && n.places.is_empty());
    }
}
