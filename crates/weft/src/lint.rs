//! Lint findings (ito-mdlint): the structure problems and the ones in the
//! Japanese text that matter (warnings and errors) are underlined in the
//! editor, `lint.next` and `lint.prev` go to them, and `lint.fix_all` mends
//! the ones with a fix as one undo step. Linting runs on the structure worker.

use ito_mdlint::{Finding, Kind, Rules, Severity};

/// Texts above this are not linted (the scan is linear, but the findings would be a wall).
pub const MAX_BYTES: usize = 4 << 20;

/// Every rule at its default severity.
pub fn rules() -> Rules {
    Rules::default()
}

/// The findings worth an underline: warnings and errors. Link targets are not
/// checked yet (every one is taken as present).
pub fn run(text: &str) -> Vec<Finding> {
    if text.len() > MAX_BYTES {
        return Vec::new();
    }
    let mut v = ito_mdlint::lint(text, &rules(), &|_| true);
    v.retain(|f| f.severity != Severity::Info);
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
        let v = run(DOC);
        assert!(v.iter().any(|f| f.kind == Kind::HeadingJump));
        assert!(v.iter().any(|f| f.kind == Kind::UnclosedFence));
        assert!(v.iter().all(|f| f.severity != Severity::Info));
        assert!(run("trailing  \nspace\n").is_empty());
    }

    #[test]
    fn stepping_wraps_round() {
        let v = run(DOC);
        assert!(v.len() >= 2);
        let last = v.last().unwrap();
        assert_eq!(step(&v, last.range.start, true), v.first());
        assert_eq!(step(&v, v[0].range.start, false), v.last());
        assert_eq!(step(&v, 0, true), v.first());
    }

    #[test]
    fn fix_all_is_one_change() {
        let text = "# A\n\n### C\n";
        let v = run(text);
        if let Some(ch) = ito_mdlint::fix_all(text, &v, None) {
            assert!(ch.range.end <= text.len());
        }
    }
}
