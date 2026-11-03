//! Characters as the status bar counts them: grapheme clusters, the steps
//! the caret takes (ito-textbuf's `pos`), so `👍🏽` and an `e` with a
//! combining accent are one each; line breaks (`\n`, `\r\n`, a lone `\r`) are
//! not counted. The document's count comes with each reading of the structure,
//! off the UI thread; the selection's is counted on the UI thread by
//! [`Selected`], once per text version and range.

use std::ops::Range;

use ito_textbuf::pos;

/// How many characters `text[range]` has. The range is clamped into the
/// text; an end inside a cluster counts that cluster.
pub fn count(text: &str, range: Range<usize>) -> usize {
    let end = range.end.min(text.len());
    let mut at = pos::floor(text, range.start.min(end));
    let mut n = 0;
    while at < end {
        let Some(next) = pos::next_grapheme(text, at) else { break };
        if !matches!(&text[at..next], "\n" | "\r\n" | "\r") {
            n += 1;
        }
        at = next;
    }
    n
}

/// The selection's count, kept between frames: the same version and range
/// is not counted again, and a selection that moved one end counts only the
/// stretch between the old end and the new (Shift+arrows in a long selection).
#[derive(Default)]
pub struct Selected {
    last: Option<(u64, Range<usize>, usize)>,
}

impl Selected {
    /// The count of `text[range]` at `version`. `whole` is the document's
    /// count at this version when one is known (a selection of everything
    /// takes it as it is).
    pub fn count(&mut self, version: u64, text: &str, range: Range<usize>, whole: Option<usize>) -> usize {
        let n = match (self.last.take(), whole) {
            (Some((v, r, n)), _) if v == version && r == range => n,
            (_, Some(w)) if range == (0..text.len()) => w,
            // One end moved: count the stretch it moved over, unless the
            // selection itself is shorter.
            (Some((v, r, n)), _) if v == version && r.start == range.start && r.end.abs_diff(range.end) < range.len() => {
                moved(n, text, r.end, range.end)
            }
            (Some((v, r, n)), _) if v == version && r.end == range.end && r.start.abs_diff(range.start) < range.len() => {
                moved(n, text, range.start, r.start)
            }
            _ => count(text, range.clone()),
        };
        self.last = Some((version, range, n));
        n
    }
}

/// `n` for a range that ended at `from` (or started at `to`), moved to end
/// at `to` (start at `from`): what lies between is added or taken away.
fn moved(n: usize, text: &str, from: usize, to: usize) -> usize {
    if to >= from {
        n + count(text, from..to)
    } else {
        n.saturating_sub(count(text, to..from))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all(text: &str) -> usize {
        count(text, 0..text.len())
    }

    /// 8.9: line breaks are not characters, whatever they are written with;
    /// full-width letters, an emoji with its skin tone and a letter with a
    /// combining accent are one each.
    #[test]
    fn characters_are_graphemes_without_the_line_breaks() {
        assert_eq!(all(""), 0);
        assert_eq!(all("abc"), 3);
        assert_eq!(all("a\nb\r\nc\rd\n"), 4);
        assert_eq!(all("\r\n\r\n"), 0);
        assert_eq!(all("日本語の文字"), 6);
        assert_eq!(all("ＡＢ　ｃ"), 4, "full-width letters and the full-width space");
        assert_eq!(all("👍🏽"), 1);
        assert_eq!(all("👨‍👩‍👧"), 1, "a family joined by ZWJ");
        assert_eq!(all("e\u{301}"), 1, "e and a combining acute");
        assert_eq!(all("か\u{3099}"), 1, "ka and a combining voiced mark");
        assert_eq!(all("# 見出し\r\n\r\n👍🏽 e\u{301}\r\n"), 8);
    }

    #[test]
    fn a_range_counts_its_own_part_and_is_kept_in_the_text() {
        let text = "ab\r\n漢字👍🏽";
        assert_eq!(count(text, 0..2), 2);
        assert_eq!(count(text, 2..4), 0);
        assert_eq!(count(text, 4..text.len()), 3);
        assert_eq!(count(text, 4..100), 3);
        assert_eq!(count(text, 50..100), 0);
        // Inside `漢` (3 bytes): the cluster it is in counts.
        assert_eq!(count(text, 5..7), 1);
    }

    /// 8.9: the selection's count follows the selection as one end moves,
    /// a change of the text counts again, and a selection of everything
    /// takes the document's count.
    #[test]
    fn the_selected_count_follows_the_selection_without_counting_it_again() {
        let text = "a\r\n漢字👍🏽e\u{301}xyz";
        let at = |s: &str| text.find(s).unwrap();
        let mut s = Selected::default();
        let mut check = |version: u64, range: Range<usize>| {
            let n = s.count(version, text, range.clone(), None);
            assert_eq!(n, count(text, range.clone()), "{range:?}");
            n
        };
        assert_eq!(check(1, 0..at("字")), 2);
        assert_eq!(check(1, 0..at("👍")), 3, "the end moved right");
        assert_eq!(check(1, 0..at("e")), 4);
        assert_eq!(check(1, 0..at("👍")), 3, "and back");
        assert_eq!(check(1, at("漢")..text.len()), 7, "both ends moved");
        assert_eq!(check(1, 1..text.len()), 7, "the start moved over the line break");
        assert_eq!(check(1, at("字")..text.len()), 6, "the start moved right");
        assert_eq!(check(2, at("字")..text.len()), 6, "a new version counts again");
        assert_eq!(check(2, at("字")..at("x")), 3, "the end moved left");
        assert_eq!(check(2, at("👍")..at("e")), 1);

        let mut s = Selected::default();
        assert_eq!(s.count(3, text, 0..text.len(), Some(99)), 99, "everything takes the document's count");
        assert_eq!(s.count(3, text, 0..at("z"), Some(99)), 98, "and goes on from it");
    }
}
