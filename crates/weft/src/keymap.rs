//! The keys: `defaults/keymap.toml` built in, the person's own `keymap.toml`
//! (in weft's folder under the uchmk one) laid over it, as koyomi's.
//!
//! Two layers. `edit` holds the keys of the document (the caret, typing's
//! plain keys, the structure commands) and works only while the document has
//! the keys; `app` works anywhere in the window. While editing, `edit` is
//! looked at first. What a key can run is a command of [`crate::commands`].

use std::sync::OnceLock;

use ito_keymap::{Binding, Layer, Match, Spec};
use ito_keys::Key;

use crate::commands;

/// The built-in keys, as the person's file is laid out.
pub const DEFAULTS: &str = include_str!("../defaults/keymap.toml");

/// The person's own file, in weft's folder.
pub const FILE: &str = "keymap.toml";

/// `run = "none"`: the key does nothing (unbinds a built-in one).
pub const NOOP: &str = "none";

const LAYERS: [Layer; 2] = [Layer::new(commands::Layer::App.name()), Layer::new(commands::Layer::Edit.name())];

pub const SPEC: Spec = Spec { layers: &LAYERS, ignored: &[], defaults: DEFAULTS };

/// Keys weft takes before the keymap sees them: the bug report, the palette,
/// the scale, and copy / cut / paste (the window turns those into clipboard
/// events before they are keys). A file that binds one is told so.
const FIXED: [&str; 9] = ["<F12>", "<C-S-p>", "<C-=>", "<C-+>", "<C-->", "<C-0>", "<C-c>", "<C-x>", "<C-v>"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cmd {
    Run(&'static str),
    None,
}

/// The `run` of a line as a command; `None` with a warning when it is no
/// command of weft.
fn command(text: &str, warnings: &mut Vec<String>) -> Option<Cmd> {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text == NOOP {
        return Some(Cmd::None);
    }
    match commands::find(&text) {
        Some(d) => Some(Cmd::Run(d.name)),
        None => {
            warnings.push(format!("keymap: `{text}` is no command of weft"));
            None
        }
    }
}

type Layers = Vec<(String, Vec<Binding<Cmd>>)>;

/// The bindings in force, by layer: `app`, then `edit`.
pub struct Keys {
    layers: Layers,
}

impl Keys {
    /// The defaults with `user` (its path for the warnings, and its text)
    /// laid over; what the file got wrong comes back as warnings.
    pub fn load(user: Option<(&str, &str)>) -> (Self, Vec<String>) {
        let files: Vec<(&str, &str)> = user.into_iter().collect();
        let mut warnings = Vec::new();
        let mut km = ito_keymap::load(&SPEC, &files, &mut |t, w| command(t, w));
        warnings.append(&mut km.warnings);
        let mut layers = Vec::new();
        for layer in LAYERS {
            let mut bindings = km.take(layer.name);
            bindings.retain(|b| match Self::fixed(&b.on) {
                Some(k) => {
                    warnings.push(format!("{}: [{}] `{}` is fixed and cannot be bound", b.from, layer.name, k.human()));
                    false
                }
                None => true,
            });
            // A line without `desc` says what its command does.
            for b in &mut bindings {
                if let ([Cmd::Run(name)], true) = (b.run.as_slice(), b.desc.is_empty()) {
                    b.desc = commands::find(name).map_or_else(String::new, |d| d.desc.to_owned());
                }
            }
            layers.push((layer.name.to_owned(), bindings));
        }
        (Self { layers }, warnings)
    }

    fn layer(&self, layer: commands::Layer) -> &[Binding<Cmd>] {
        self.layers.iter().find(|(n, _)| n == layer.name()).map_or(&[], |(_, b)| b.as_slice())
    }

    /// The layers as the settings' Keys page lists them.
    pub fn layers(&self) -> &[(String, Vec<Binding<Cmd>>)] {
        &self.layers
    }

    /// The built-in keys alone, for the Keys page to show what a change
    /// departs from.
    pub fn built_in_layers() -> &'static [(String, Vec<Binding<Cmd>>)] {
        static BUILT_IN: OnceLock<Layers> = OnceLock::new();
        BUILT_IN.get_or_init(|| Self::load(None).0.layers)
    }

    /// The key in `keys` that weft takes before the keymap, if any.
    pub fn fixed(keys: &[Key]) -> Option<&Key> {
        keys.iter().find(|k| FIXED.iter().any(|f| Key::parse(f).as_ref() == Some(k)))
    }

    /// What `pressed` (the chord so far) runs. While `editing`, the edit
    /// layer is looked at first, then the app one.
    pub fn resolve(&self, pressed: &[Key], editing: bool) -> Match<'_, Cmd> {
        if editing {
            let m = ito_keymap::resolve(self.layer(commands::Layer::Edit), pressed);
            if !matches!(m, Match::None) {
                return m;
            }
        }
        ito_keymap::resolve(self.layer(commands::Layer::App), pressed)
    }

    /// The keys that run `name`, as the palette shows them; empty when it is
    /// unbound (or every key of it is shadowed).
    pub fn label(&self, name: &str) -> String {
        let editing = commands::find(name).is_some_and(|d| d.layer == commands::Layer::Edit);
        self.layers
            .iter()
            .flat_map(|(_, b)| b)
            .filter(|b| matches!(b.run.as_slice(), [Cmd::Run(n)] if *n == name))
            .find(|b| matches!(self.resolve(&b.on, editing), Match::Exact(e) if std::ptr::eq(e, *b)))
            .map(|b| ito_keys::human_seq(&b.on))
            .unwrap_or_default()
    }
}

/// The commands no built-in key runs, for the Keys page to offer:
/// `(layer, run, desc)`.
pub fn unbound() -> &'static [(&'static str, &'static str, &'static str)] {
    static UNBOUND: OnceLock<Vec<(&'static str, &'static str, &'static str)>> = OnceLock::new();
    UNBOUND.get_or_init(|| {
        let bound = |name: &str| {
            Keys::built_in_layers().iter().flat_map(|(_, b)| b).any(|b| matches!(b.run.as_slice(), [Cmd::Run(n)] if *n == name))
        };
        commands::ALL.iter().filter(|d| !bound(d.name)).map(|d| (d.layer.name(), d.name, d.desc)).collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(k: &Keys, on: &str, editing: bool) -> Option<Vec<Cmd>> {
        let on: Vec<Key> = on.split_whitespace().map(|t| Key::parse(t).unwrap()).collect();
        match k.resolve(&on, editing) {
            Match::Exact(b) => Some(b.run.clone()),
            _ => None,
        }
    }

    #[test]
    fn the_defaults_load_without_a_word_and_bind_the_app_commands() {
        let (k, warnings) = Keys::load(None);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(run(&k, "<C-q>", false), Some(vec![Cmd::Run("quit")]));
        assert_eq!(run(&k, "<F1>", true), Some(vec![Cmd::Run("help")]), "app keys work while editing");
        assert_eq!(run(&k, "<C-,>", false), Some(vec![Cmd::Run("settings")]));
        assert_eq!(run(&k, "<C-s>", true), Some(vec![Cmd::Run("file.save")]));
        assert_eq!(run(&k, "<A-Up>", true), Some(vec![Cmd::Run("block.move_up")]));
        assert_eq!(run(&k, "<A-Up>", false), None, "edit keys only while editing");
        assert_eq!(run(&k, "q", true), None);
        assert_eq!(k.label("quit"), "Ctrl+Q");
        assert_eq!(k.label("block.move_up"), "Alt+Up");
    }

    #[test]
    fn every_command_has_a_built_in_key_or_is_offered_unbound() {
        let (k, _) = Keys::load(None);
        for d in commands::ALL {
            let offered = unbound().iter().any(|(l, r, _)| *r == d.name && *l == d.layer.name());
            assert!(!k.label(d.name).is_empty() || offered, "{} has no key", d.name);
            assert!(k.label(d.name).is_empty() || !offered, "{} has a key and is offered", d.name);
        }
        assert!(unbound().iter().any(|(_, r, _)| *r == "table.format"));
    }

    #[test]
    fn the_built_in_keys_are_in_their_commands_layer_and_say_what_it_does() {
        for (layer, bindings) in Keys::built_in_layers() {
            for b in bindings {
                let [Cmd::Run(name)] = b.run.as_slice() else { panic!("{}: {:?}", b.raw, b.run) };
                let d = commands::find(name).unwrap();
                assert_eq!(d.layer.name(), layer, "{name}");
                assert_eq!(b.desc, d.desc, "{name}: the default's desc is the table's");
            }
        }
    }

    #[test]
    fn the_built_in_layers_are_the_defaults_once() {
        let b = Keys::built_in_layers();
        let names: Vec<&str> = b.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["app", "edit"]);
        assert_eq!(b[0].1.len(), Keys::load(None).0.layers()[0].1.len());
        assert!(std::ptr::eq(b, Keys::built_in_layers()));
    }

    #[test]
    fn a_file_unbinds_rebinds_and_is_told_what_it_got_wrong() {
        let text = "[[app.keymap]]\non = \"<C-q>\"\nrun = \"none\"\n\n[[app.keymap]]\non = \"<C-w>\"\nrun = \"quit\"\n\n\
                    [[app.keymap]]\non = \"<F12>\"\nrun = \"help\"\n\n[[app.keymap]]\non = \"<C-k>\"\nrun = \"explode\"\n\n\
                    [[edit.keymap]]\non = \"<C-v>\"\nrun = \"select.all\"\n\n[[edit.keymap]]\non = \"<C-l>\"\nrun = \"table.format\"\n";
        let (k, warnings) = Keys::load(Some(("keymap.toml", text)));
        assert_eq!(run(&k, "<C-q>", false), Some(vec![Cmd::None]));
        assert_eq!(run(&k, "<C-w>", false), Some(vec![Cmd::Run("quit")]));
        assert_eq!(run(&k, "<F12>", false), None);
        assert_eq!(run(&k, "<C-k>", false), None);
        assert_eq!(run(&k, "<C-v>", true), None, "paste is fixed");
        assert_eq!(run(&k, "<C-l>", true), Some(vec![Cmd::Run("table.format")]));
        assert_eq!(k.label("quit"), "Ctrl+W");
        assert_eq!(k.label("table.format"), "Ctrl+L");
        let own = k.layers()[1].1.iter().find(|b| b.on == [Key::parse("<C-l>").unwrap()]).unwrap();
        assert_eq!(own.desc, "Format the table", "a line without desc takes the command's");
        assert!(warnings.iter().any(|w| w.contains("[app]") && w.contains("F12")), "{warnings:?}");
        assert!(warnings.iter().any(|w| w.contains("[edit]") && w.contains("Ctrl+V")), "{warnings:?}");
        assert!(warnings.iter().any(|w| w.contains("explode")), "{warnings:?}");
    }
}
