//! Folded headings: the lines of a folded section are not shown. The fold is
//! kept by the heading (its place in the outline and its title), so editing
//! elsewhere keeps it; the lines it hides are worked out from the newest
//! reading of the structure, and only when that reading is of the text as it
//! is now (until then the last ones stay).

use std::ops::Range;

use crate::engine::Editor;
use crate::structure::Tree;

/// One folded heading.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Key {
    index: usize,
    level: u8,
    title: String,
}

/// A fold's effect: the heading's line, and the lines below it that are hidden.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hidden {
    /// The heading's place in the outline.
    pub index: usize,
    pub heading: usize,
    pub lines: Range<usize>,
}

#[derive(Default)]
pub struct Folds {
    keys: Vec<Key>,
    /// By line, not overlapping.
    hidden: Vec<Hidden>,
    /// The version of the text `hidden` was worked out for.
    stamp: Option<u64>,
}

impl Folds {
    /// The hidden lines, in order (what the view lays out around).
    pub fn hidden(&self) -> &[Hidden] {
        &self.hidden
    }

    /// The fold whose hidden lines include `line`.
    fn covering(&self, line: usize) -> Option<usize> {
        let i = self.hidden.partition_point(|h| h.lines.end <= line);
        self.hidden.get(i).filter(|h| h.lines.contains(&line)).map(|_| i)
    }

    /// How many lines the heading on `line` hides, if it is folded.
    pub fn hides(&self, line: usize) -> Option<usize> {
        let i = self.hidden.binary_search_by_key(&line, |h| h.heading).ok()?;
        Some(self.hidden[i].lines.len())
    }

    /// Fold or unfold the heading `i` of `tree`.
    pub fn toggle(&mut self, tree: &Tree, i: usize) {
        let Some(h) = tree.headings.get(i) else { return };
        if let Some(p) = self.keys.iter().position(|k| k.index == i && k.title == h.title) {
            self.keys.remove(p);
        } else {
            self.keys.push(Key { index: i, level: h.level, title: h.title.clone() });
        }
        self.stamp = None;
    }

    /// Fold every heading that has something below it, or unfold all.
    pub fn set_all(&mut self, tree: &Tree, fold: bool) {
        self.keys.clear();
        if fold {
            for (i, h) in tree.headings.iter().enumerate() {
                let body = h.section.end > h.range.end + 1;
                if body {
                    self.keys.push(Key { index: i, level: h.level, title: h.title.clone() });
                }
            }
        }
        self.stamp = None;
    }

    /// Unfold whatever hides `line` (the caret is there). True if one did.
    pub fn reveal(&mut self, line: usize) -> bool {
        let mut any = false;
        while let Some(i) = self.covering(line) {
            let index = self.hidden.remove(i).index;
            self.keys.retain(|k| k.index != index);
            self.stamp = None;
            any = true;
        }
        any
    }

    /// Work `hidden` out again if `tree` is a reading of the text as it is.
    pub fn refresh(&mut self, tree: Option<&Tree>, ed: &Editor) {
        let Some(tree) = tree.filter(|t| t.version == ed.version()) else { return };
        if self.stamp == Some(tree.version) {
            return;
        }
        self.stamp = Some(tree.version);
        // Keep each fold on its heading: the same place, or the nearest heading of the same level and title.
        let mut kept: Vec<Key> = Vec::new();
        for k in &self.keys {
            let same = |i: usize| tree.headings.get(i).is_some_and(|h| h.level == k.level && h.title == k.title);
            let found = if same(k.index) {
                Some(k.index)
            } else {
                (0..tree.headings.len()).filter(|&i| same(i)).min_by_key(|&i| i.abs_diff(k.index))
            };
            if let Some(index) = found {
                if !kept.iter().any(|x| x.index == index) {
                    kept.push(Key { index, ..k.clone() });
                }
            }
        }
        kept.sort_by_key(|k| k.index);
        self.keys = kept;
        let text = ed.text();
        let mut hidden: Vec<Hidden> = Vec::new();
        for k in &self.keys {
            let h = &tree.headings[k.index];
            let first = h.line + 1;
            // The line the section ends before (a section ending at the very end covers the last line too).
            let end = if h.section.end >= text.len() { ed.line_count() } else { ed.line_of(h.section.end) };
            // A heading inside an earlier fold adds nothing.
            let from = hidden.last().map_or(first, |p| first.max(p.lines.end));
            if end > from {
                hidden.push(Hidden { index: k.index, heading: h.line, lines: from..end });
            }
        }
        self.hidden = hidden;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "# A

intro

## B

body

## C

c

# D

d
";

    fn folds(doc: &str, fold: &[usize]) -> (Folds, Tree, Editor) {
        let ed = Editor::new(doc);
        let tree = crate::structure::read(ed.version(), doc);
        let mut f = Folds::default();
        for &i in fold {
            f.toggle(&tree, i);
        }
        f.refresh(Some(&tree), &ed);
        (f, tree, ed)
    }

    #[test]
    fn a_folded_section_hides_the_lines_below_its_heading() {
        let (f, _, _) = folds(DOC, &[1]);
        // "## B" is line 4; its section runs to "## C" at line 8.
        assert_eq!(f.hidden(), &[Hidden { index: 1, heading: 4, lines: 5..8 }]);
        assert_eq!(f.hides(4), Some(3));
        assert_eq!(f.hides(0), None);
    }

    #[test]
    fn a_fold_inside_a_fold_adds_nothing_and_the_last_section_reaches_the_end() {
        let (f, _, ed) = folds(DOC, &[0, 1, 3]);
        assert_eq!(f.hidden()[0].lines, 1..12);
        assert_eq!(f.hidden().len(), 2);
        assert_eq!(f.hidden()[1].lines, 13..ed.line_count());
    }

    #[test]
    fn a_caret_in_hidden_lines_opens_the_fold() {
        let (mut f, _, _) = folds(DOC, &[1]);
        assert!(!f.reveal(4));
        assert!(f.reveal(6));
        assert!(f.hidden().is_empty());
    }

    #[test]
    fn a_fold_stays_on_its_heading_when_one_is_added_above() {
        let (mut f, _, _) = folds(DOC, &[1]);
        let doc = format!(
            "# Z

{DOC}"
        );
        let ed = Editor::new(&doc);
        let tree = crate::structure::read(ed.version(), &doc);
        f.stamp = None;
        f.refresh(Some(&tree), &ed);
        assert_eq!(f.hidden()[0].index, 2);
        assert_eq!(f.hidden()[0].heading, 6);
    }

    #[test]
    fn all_sections_fold_and_unfold() {
        let (mut f, tree, ed) = folds(DOC, &[]);
        f.set_all(&tree, true);
        f.refresh(Some(&tree), &ed);
        assert!(f.hides(0).is_some() && f.hides(12).is_some());
        f.set_all(&tree, false);
        f.refresh(Some(&tree), &ed);
        assert!(f.hidden().is_empty());
    }
}
