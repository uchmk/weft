//! Lint findings (ito-mdlint): the structure problems and the ones in the
//! Japanese text that matter (warnings and errors) are underlined in the
//! editor, `lint.next` and `lint.prev` go to them, and `lint.fix_all` mends
//! the ones with a fix as one undo step. Linting runs on the structure worker.

use ito_mdlint::{Finding, Kind, Punctuation, Rules, Severity, Spacing, Tone};

/// Texts above this are not linted (the scan is linear, but the findings would be a wall).
pub const MAX_BYTES: usize = 4 << 20;

/// What `[lint]` of config.toml says: whether the quiet (info) findings are
/// shown, and the options of the Japanese checks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cfg {
    pub info: bool,
    pub spacing: Spacing,
    pub punctuation: Punctuation,
    pub tone: Tone,
}

impl Cfg {
    fn rules(&self) -> Rules {
        let mut r = Rules::default();
        (r.spacing, r.punctuation, r.tone) = (self.spacing, self.punctuation, self.tone);
        r
    }
}

#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct File {
    lint: Table,
}

#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct Table {
    info: Option<toml::Value>,
    spacing: Option<String>,
    punctuation: Option<String>,
    tone: Option<String>,
}

/// `[lint]` of the config.toml `text` (none: no file), with what to warn of.
/// A value that is not one of the choices is said, and the default stands in
/// for it. (A file that does not read is said once, by the `[notify]` reading.)
pub fn read(text: Option<&str>) -> (Cfg, Vec<String>) {
    let mut cfg = Cfg::default();
    let mut warn = Vec::new();
    let Some(Ok(f)) = text.map(toml::from_str::<File>) else { return (cfg, warn) };
    let t = f.lint;
    match t.info {
        Some(toml::Value::Boolean(on)) => cfg.info = on,
        Some(v) => warn.push(format!("config.toml: [lint] info = {v} is not true or false; false is used")),
        None => {}
    }
    let mut pick = |key: &str, v: Option<String>, choices: &[&str]| -> Option<usize> {
        let v = v?;
        let i = choices.iter().position(|c| *c == v);
        if i.is_none() {
            warn.push(format!("config.toml: [lint] {key} = \"{v}\" is not {}; the default is used", choices.join(", ")));
        }
        i
    };
    match pick("spacing", t.spacing, &["any", "space", "no_space"]) {
        Some(1) => cfg.spacing = Spacing::Space,
        Some(2) => cfg.spacing = Spacing::NoSpace,
        _ => {}
    }
    match pick("punctuation", t.punctuation, &["auto", "ideographic", "western"]) {
        Some(1) => cfg.punctuation = Punctuation::Ideographic,
        Some(2) => cfg.punctuation = Punctuation::Western,
        _ => {}
    }
    match pick("tone", t.tone, &["auto", "polite", "plain"]) {
        Some(1) => cfg.tone = Tone::Polite,
        Some(2) => cfg.tone = Tone::Plain,
        _ => {}
    }
    (cfg, warn)
}

/// The findings worth an underline: warnings and errors (and the info ones
/// when `cfg` asks). Relative link targets are looked for in `base` (the
/// note's folder); with no folder (a note not saved yet) every one is taken
/// as present.
pub fn run(text: &str, base: Option<&std::path::Path>, cfg: &Cfg) -> Vec<Finding> {
    if text.len() > MAX_BYTES {
        return Vec::new();
    }
    let exists = |target: &str| base.is_none_or(|b| b.join(target).exists());
    let mut v = ito_mdlint::lint(text, &cfg.rules(), &exists);
    if !cfg.info {
        v.retain(|f| f.severity != Severity::Info);
    }
    v
}

/// The first finding that starts after `caret` (wrapping), or the last that starts before it.
pub fn step(findings: &[Finding], caret: usize, forward: bool) -> Option<&Finding> {
    if forward {
        findings.iter().find(|f| f.range.start > caret).or_else(|| findings.first())
    } else {
        findings.iter().rev().find(|f| f.range.start < caret).or_else(|| findings.last())
    }
}

/// The language key of a finding's kind.
pub fn key(kind: Kind) -> String {
    format!("lint.{}", kind.key())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "# A\n\n### C\n\ntext\n\n```\nopen\n";

    #[test]
    fn warnings_and_errors_are_found_and_info_is_left_out() {
        let v = run(DOC, None, &Cfg::default());
        assert!(v.iter().any(|f| f.kind == Kind::HeadingJump));
        assert!(v.iter().any(|f| f.kind == Kind::UnclosedFence));
        assert!(v.iter().all(|f| f.severity != Severity::Info));
        assert!(run("trailing  \nspace\n", None, &Cfg::default()).is_empty());
    }

    #[test]
    fn the_lint_table_is_read_and_a_wrong_value_is_said() {
        assert_eq!(read(None), (Cfg::default(), vec![]));
        let (c, w) = read(Some(
            "[lint]
info = true
spacing = \"space\"
tone = \"plain\"
",
        ));
        assert!(w.is_empty());
        assert!(c.info && c.spacing == Spacing::Space && c.tone == Tone::Plain && c.punctuation == Punctuation::Auto);
        let (c, w) = read(Some(
            "[lint]
info = 3
punctuation = \"x\"
",
        ));
        assert_eq!(c, Cfg::default());
        assert_eq!(w.len(), 2);
    }

    #[test]
    fn info_findings_show_only_when_asked() {
        let text = "trailing   
space
";
        let on = Cfg { info: true, ..Cfg::default() };
        assert!(run(text, None, &Cfg::default()).is_empty());
        assert!(run(text, None, &on).iter().all(|f| f.severity == Severity::Info) && !run(text, None, &on).is_empty());
    }

    #[test]
    fn stepping_wraps_round() {
        let v = run(DOC, None, &Cfg::default());
        assert!(v.len() >= 2);
        let last = v.last().unwrap();
        assert_eq!(step(&v, last.range.start, true), v.first());
        assert_eq!(step(&v, v[0].range.start, false), v.last());
        assert_eq!(step(&v, 0, true), v.first());
    }

    #[test]
    fn a_link_to_a_missing_file_is_found_only_when_the_folder_is_known() {
        let dir = std::env::temp_dir().join(format!("weft-lint-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("here.md"), "x").unwrap();
        let text = "[a](here.md) [b](gone.md)
";
        assert!(run(text, None, &Cfg::default()).is_empty());
        let v = run(text, Some(&dir), &Cfg::default());
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].kind, Kind::MissingTarget);
        assert_eq!(&text[v[0].range.clone()], "gone.md");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fix_all_is_one_change() {
        let text = "# A\n\n### C\n";
        let v = run(text, None, &Cfg::default());
        if let Some(ch) = ito_mdlint::fix_all(text, &v, None) {
            assert!(ch.range.end <= text.len());
        }
    }
}
