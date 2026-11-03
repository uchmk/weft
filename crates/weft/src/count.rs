//! Characters as the status bar counts them: grapheme clusters, the steps
//! the caret takes (ito-textbuf's `pos`), so `👍🏽` and an `e` with a
//! combining accent are one each; line breaks (`\n`, `\r\n`, a lone `\r`) are
//! not counted.
//!
//! The document's count comes with each reading of the structure, off the UI
//! thread, with an [`Index`]: the count at a cluster boundary about every
//! [`STEP`] bytes. A selection's count is the difference of the counts before
//! its ends, so with a reading of the text as it is the UI thread walks at
//! most a step at each end, whatever the selection's size. While the reading
//! is behind, [`Selected`] walks at most [`WALK`] bytes (a short selection,
//! or the stretch one end moved) and otherwise says the count is pending.

use std::ops::Range;

use ito_textbuf::pos;

/// Bytes between the marks of an [`Index`] (a mark is at the first cluster
/// boundary this far past the last): what is walked, at most, past a mark.
pub const STEP: usize = 1024;

/// Bytes [`Selected`] may walk on the UI thread to count without a reading of
/// the text as it is (well under a millisecond in a release build).
pub const WALK: usize = 32 * 1024;

#[cfg(test)]
thread_local! {
    /// Bytes [`count`] walked on this thread (the tests' proof that a count
    /// did not walk the selection).
    static WALKED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// What `f` returns, and the bytes it made [`count`] walk on this thread.
#[cfg(test)]
pub fn walked<T>(f: impl FnOnce() -> T) -> (T, usize) {
    let before = WALKED.with(|w| w.get());
    let out = f();
    (out, WALKED.with(|w| w.get()) - before)
}

/// How many characters `text[range]` has. The range is clamped into the
/// text; an end inside a cluster counts that cluster (ito's `count_chars`
/// would count only the part inside, so the ends are moved out to the
/// clusters first: a stretch from boundary to boundary is cut into the
/// clusters the whole text has there).
pub fn count(text: &str, range: Range<usize>) -> usize {
    let end = range.end.min(text.len());
    let start = pos::floor(text, range.start.min(end));
    if start >= end {
        return 0;
    }
    let (from, to) = (cluster_start(text, start), cluster_end(text, end));
    #[cfg(test)]
    WALKED.with(|w| w.set(w.get() + (to - from)));
    pos::count_chars(text, from..to)
}

/// The start of the cluster `at` is in (`at` itself on a boundary).
fn cluster_start(text: &str, at: usize) -> usize {
    let at = pos::floor(text, at);
    match pos::prev_grapheme(text, at) {
        Some(p) if pos::next_grapheme(text, p) != Some(at) => p,
        _ => at,
    }
}

/// The first cluster boundary at or past `at` (`at` itself on a boundary,
/// else the end of the cluster `at` is in).
fn cluster_end(text: &str, at: usize) -> usize {
    let floor = pos::floor(text, at);
    if floor == at && cluster_start(text, at) == at {
        at
    } else {
        pos::next_grapheme(text, floor).unwrap_or(text.len())
    }
}

/// `range` clamped into the text with its ends moved out to the clusters
/// they are in, so it counts as [`count`] counts it: the same count, from
/// boundary to boundary. An empty range stays empty.
pub fn snap(text: &str, range: Range<usize>) -> Range<usize> {
    let end = range.end.min(text.len());
    let start = range.start.min(end);
    if start == end {
        return start..start;
    }
    let to = pos::floor(text, end);
    let to = if cluster_start(text, to) == to { to } else { pos::next_grapheme(text, to).unwrap_or(text.len()) };
    cluster_start(text, start)..to
}

/// The count before a cluster boundary about every [`STEP`] bytes of one
/// text, made with the structure reading off the UI thread.
#[derive(Clone, Debug)]
pub struct Index {
    /// `(byte, characters before it)`, from `(0, 0)`, in order; each byte a
    /// cluster boundary.
    marks: Vec<(usize, usize)>,
    /// The text's length, and its count.
    len: usize,
    total: usize,
}

impl Index {
    /// Count `text` whole, marking the way.
    pub fn new(text: &str) -> Index {
        let mut marks = vec![(0, 0)];
        let (mut n, mut mark) = (0, 0);
        loop {
            let goal = mark + STEP;
            if goal > text.len() {
                n += pos::count_chars(text, mark..text.len());
                break;
            }
            let next = cluster_end(text, goal);
            n += pos::count_chars(text, mark..next);
            marks.push((next, n));
            mark = next;
        }
        Index { marks, len: text.len(), total: n }
    }

    /// The characters in the whole text.
    pub fn total(&self) -> usize {
        self.total
    }

    /// The characters before `at`, a cluster boundary of the text.
    fn before(&self, text: &str, at: usize) -> usize {
        if at >= self.len {
            return self.total;
        }
        let i = self.marks.partition_point(|&(b, _)| b <= at) - 1;
        let (b, n) = self.marks[i];
        n + count(text, b..at)
    }

    /// The count of `text[range]` as [`count`] counts it, `text` being the
    /// text this index was made from; at most a step is walked at each end.
    pub fn count(&self, text: &str, range: Range<usize>) -> usize {
        debug_assert_eq!(text.len(), self.len, "an index of another text");
        let r = snap(text, range);
        if r.is_empty() {
            return 0;
        }
        self.before(text, r.end) - self.before(text, r.start)
    }
}

/// The selection's count, kept between frames: the same version and range
/// is not counted again, a selection that moved one end a little counts only
/// the stretch between the old end and the new (Shift+arrows in a long
/// selection), and any other is taken from the reading's [`Index`] or,
/// without one, walked when short and otherwise pending. Ends inside a
/// cluster are moved out to it first ([`snap`]), so the stretches add up to
/// what a count of the whole selection says.
#[derive(Default)]
pub struct Selected {
    last: Option<(u64, Range<usize>, usize)>,
}

impl Selected {
    /// The count of `text[range]` at `version`, or `None` while it is
    /// pending. `reading` is the index of this version's text, when the
    /// structure worker has read it. Without one, no more than [`WALK`] bytes
    /// are walked: a longer count waits for the reading.
    pub fn count(&mut self, version: u64, text: &str, range: Range<usize>, reading: Option<&Index>) -> Option<usize> {
        let range = snap(text, range);
        let last = self.last.take();
        let n = match &last {
            Some((v, r, n)) if *v == version && *r == range => Some(*n),
            // One end moved a little: count the stretch it moved over, unless
            // the selection itself is shorter.
            Some((v, r, n)) if *v == version && r.start == range.start && short(r.end.abs_diff(range.end), &range) => {
                Some(moved(*n, text, r.end, range.end))
            }
            Some((v, r, n)) if *v == version && r.end == range.end && short(r.start.abs_diff(range.start), &range) => {
                Some(moved(*n, text, range.start, r.start))
            }
            _ => match reading {
                Some(index) => Some(index.count(text, range.clone())),
                None if range.len() <= WALK => Some(count(text, range.clone())),
                None => None,
            },
        };
        self.last = match n {
            Some(n) => Some((version, range, n)),
            // Pending: the last count stays to go on from.
            None => last,
        };
        n
    }
}

/// A stretch of `walk` bytes is worth walking for `range`: shorter than the
/// selection and than [`WALK`].
fn short(walk: usize, range: &Range<usize>) -> bool {
    walk < range.len() && walk <= WALK
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
            let n = s.count(version, text, range.clone(), None).expect("a short text is counted");
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

        // A reading that says the document has 99.
        let whole = Index { marks: vec![(0, 0)], len: text.len(), total: 99 };
        let mut s = Selected::default();
        assert_eq!(s.count(3, text, 0..text.len(), Some(&whole)), Some(99), "everything takes the document's count");
        assert_eq!(s.count(3, text, 0..at("z"), Some(&whole)), Some(98), "and goes on from it");
    }

    /// An end inside a cluster is moved out to it, so moving one end adds up
    /// to what a count of the selection says (every range on a `char`
    /// boundary, from every range that shares an end with it).
    #[test]
    fn the_selected_count_agrees_with_a_full_count_when_an_end_is_inside_a_cluster() {
        let text = "か\u{3099}\r\n\u{3000}a#👍🏽#👨‍👩‍👧 ";
        // The verifier's case: both ends inside the family.
        assert_eq!(count(text, 26..36), 1);
        assert_eq!(snap(text, 26..36), 22..40);
        let mut s = Selected::default();
        assert_eq!(s.count(1, text, 26..text.len(), None), Some(2));
        assert_eq!(s.count(1, text, 26..36, None), Some(1), "the end moved into the family it starts in");
        let mut s = Selected::default();
        assert_eq!(s.count(1, text, 13..36, None), Some(3));
        assert_eq!(s.count(1, text, 26..36, None), Some(1), "the start moved into the family");

        let ends: Vec<usize> = (0..=text.len()).filter(|&b| text.is_char_boundary(b)).collect();
        let index = Index::new(text);
        for &a in &ends {
            for &b in ends.iter().filter(|&&b| b > a) {
                assert_eq!(index.count(text, a..b), count(text, a..b), "{a}..{b} by the index");
                for &m in &ends {
                    for next in [a.min(m)..a.max(m), m.min(b)..m.max(b)] {
                        if next.is_empty() {
                            continue;
                        }
                        let mut s = Selected::default();
                        s.count(1, text, a..b, None);
                        assert_eq!(s.count(1, text, next.clone(), None), Some(count(text, next.clone())), "{a}..{b} then {next:?}");
                    }
                }
            }
        }
    }

    /// The text of `n` bytes or more: lines of headings, Japanese, emoji and
    /// combining marks, with CRLF.
    fn long(n: usize) -> String {
        let line = "# 見出し\r\n\r\nSome **bold** `code`、日本語の文。👍🏽 e\u{301} か\u{3099} 👨‍👩‍👧\r\n";
        line.repeat(n / line.len() + 1)
    }

    /// 8.9: with a reading of the text as it is, a long selection's count is a
    /// full count's, and only a step at each end is walked to get it.
    #[test]
    fn a_long_selection_is_counted_from_the_reading_without_walking_it() {
        let text = long(512 << 10);
        let index = Index::new(&text);
        assert_eq!(index.total(), all(&text));
        for range in [0..text.len(), 1..text.len() - 1, text.len() / 3..text.len() * 2 / 3, 7..8, 5..5] {
            let (n, walk) = walked(|| index.count(&text, range.clone()));
            assert_eq!(n, count(&text, range.clone()), "{range:?}");
            assert!(walk <= 2 * (STEP + 64), "{range:?} walked {walk} bytes");
        }
        let mut s = Selected::default();
        let range = 10..text.len() - 10;
        let (n, walk) = walked(|| s.count(1, &text, range.clone(), Some(&index)));
        assert_eq!(n, Some(count(&text, range)));
        assert!(walk <= 2 * (STEP + 64), "walked {walk} bytes");
    }

    /// 8.9: while the reading is behind, a long selection is pending, not
    /// counted; a short one, and a short move of one end, are counted; the
    /// count comes once a reading of this text does.
    #[test]
    fn a_long_selection_without_a_current_reading_is_pending() {
        let text = long(256 << 10);
        let mut s = Selected::default();
        let (n, walk) = walked(|| s.count(1, &text, 0..text.len(), None));
        assert_eq!((n, walk), (None, 0), "Ctrl+A before the reading");
        let (n, walk) = walked(|| s.count(1, &text, 100..text.len() - 100, None));
        assert_eq!((n, walk), (None, 0), "a large new selection");

        let near = 100..WALK / 2;
        assert_eq!(s.count(1, &text, near.clone(), None), Some(count(&text, near.clone())), "a short one is counted");
        let further = 100..WALK - 100;
        let (n, walk) = walked(|| s.count(1, &text, further.clone(), None));
        assert_eq!(n, Some(count(&text, further.clone())), "and an end moved a little");
        assert!(walk <= WALK / 2 + 64, "only the stretch it moved: {walk}");
        let (n, walk) = walked(|| s.count(1, &text, 100..text.len(), None));
        assert_eq!((n, walk), (None, 0), "an end moved far");
        let (n, walk) = walked(|| s.count(2, &text, further.clone(), None));
        assert_eq!(n, Some(count(&text, further)), "a short one at a new version");
        assert!(walk <= WALK, "{walk}");

        let (n, walk) = walked(|| s.count(3, &text, 100..text.len(), None));
        assert_eq!((n, walk), (None, 0), "an edit with a long selection kept");
        let index = Index::new(&text);
        assert_eq!(s.count(3, &text, 100..text.len(), Some(&index)), Some(count(&text, 100..text.len())), "the reading came");
    }
}
