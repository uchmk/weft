//! The structure of the document, read off the UI thread: the headings
//! (ito-outline), the colour of every byte, by line, the trailing spaces that
//! make a hard line break, with the options Markdown is read with in uchmk,
//! and the count of characters the status bar shows (with an index to count a
//! selection from). Each reading carries the
//! version of the text it was read from; the worker drops jobs a newer one
//! overtook, and the window keeps the colours it has until a newer reading
//! comes back.

use std::ops::Range;
use std::sync::mpsc::{self, Receiver, Sender};

use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};

/// What a run of bytes is, for its colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Heading,
    Emphasis,
    Strong,
    Strike,
    /// Inline code, an indented block, a fence of a language ito-syntax does not know.
    Code,
    /// Links, images and footnote references.
    Link,
    Quote,
    /// A list item's marker (`-`, `1.`).
    Marker,
    /// A task's box (`[ ]`, `[x]`).
    Task,
    Rule,
    /// Front matter.
    Meta,
    Html,
    /// A code fence's own lines (```` ```rust ````).
    Fence,
    /// A token of fenced code.
    Syntax(ito_syntax::Tok),
}

impl Kind {
    pub fn color(self, c: &ito_theme::Colors) -> egui::Color32 {
        use ito_syntax::Tok;
        match self {
            Kind::Heading => c.blue,
            Kind::Emphasis => c.magenta,
            Kind::Strong => c.strong(),
            Kind::Strike | Kind::Quote | Kind::Rule | Kind::Meta | Kind::Html => c.dim,
            Kind::Code => c.done,
            Kind::Link => c.run,
            Kind::Marker | Kind::Task => c.wait,
            Kind::Fence => c.faint(),
            Kind::Syntax(t) => match t {
                Tok::Keyword => c.magenta,
                Tok::Str => c.done,
                Tok::Comment => c.dim,
                Tok::Number | Tok::Const => c.wait,
                Tok::Type => c.blue,
                Tok::Func => c.run,
            },
        }
    }
}

/// One line's runs: byte ranges from the line's start, in order and apart.
pub type Runs = Vec<(Range<usize>, Kind)>;

/// One reading of the text.
pub struct Tree {
    /// The version of the text this was read from.
    pub version: u64,
    pub headings: Vec<ito_outline::Heading>,
    /// By line (split at `\n`, a `\r` before it left out).
    pub lines: Vec<Runs>,
    /// The spaces (or tabs) at the end of a line that Markdown reads as a
    /// hard line break, easy to leave by mistake and not seen: `(line, bytes
    /// from the line's start)`, by line, one at most per line. A backslash
    /// break is left out, as it shows.
    pub breaks: Vec<(usize, Range<usize>)>,
    /// The characters in the text, as the status bar counts them ([`crate::count`]).
    pub chars: usize,
    /// Where the count stands through the text, so a selection of this
    /// version is counted without walking it on the UI thread.
    pub counts: crate::count::Index,
}

impl Tree {
    /// Line `n`'s hard-break spaces, if it ends in some.
    pub fn hard_break(&self, n: usize) -> Option<Range<usize>> {
        let i = self.breaks.binary_search_by_key(&n, |(line, _)| *line).ok()?;
        Some(self.breaks[i].1.clone())
    }
}

/// Read `text` (at `version`) whole.
pub fn read(version: u64, text: &str) -> Tree {
    let (kinds, breaks) = paint(text);
    let counts = crate::count::Index::new(text);
    Tree {
        version,
        headings: ito_outline::headings(text, 6),
        lines: lines(text, &kinds),
        breaks: by_line(text, &breaks),
        chars: counts.total(),
        counts,
    }
}

fn fill(kinds: &mut [Option<Kind>], r: Range<usize>, k: Option<Kind>) {
    let end = r.end.min(kinds.len());
    if r.start < end {
        kinds[r.start..end].fill(k);
    }
}

/// The kind of every byte, and the hard breaks made of trailing spaces (the
/// spaces' bytes, in order). Elements come in document order, the outer
/// before the inner, so the inner one's colour wins.
fn paint(text: &str) -> (Vec<Option<Kind>>, Vec<Range<usize>>) {
    let mut kinds = vec![None; text.len()];
    let mut breaks = Vec::new();
    // The fence being read: its stream (None for a language not known) and its content's pieces.
    let mut fence: Option<(Option<ito_syntax::Stream>, Vec<Range<usize>>)> = None;
    for (ev, r) in Parser::new_ext(text, ito_outline::options()).into_offset_iter() {
        match ev {
            Event::Start(tag) => {
                let k = match tag {
                    Tag::Heading { .. } => Kind::Heading,
                    Tag::Emphasis => Kind::Emphasis,
                    Tag::Strong => Kind::Strong,
                    Tag::Strikethrough => Kind::Strike,
                    Tag::Link { .. } | Tag::Image { .. } => Kind::Link,
                    Tag::BlockQuote(_) => Kind::Quote,
                    Tag::MetadataBlock(_) => Kind::Meta,
                    Tag::HtmlBlock => Kind::Html,
                    Tag::CodeBlock(CodeBlockKind::Indented) => Kind::Code,
                    Tag::CodeBlock(CodeBlockKind::Fenced(info)) => {
                        fence = Some((ito_syntax::Stream::for_fence(&info), Vec::new()));
                        Kind::Fence
                    }
                    Tag::Item => {
                        let line = &text[r.clone()];
                        let lead = line.len() - line.trim_start_matches([' ', '\t']).len();
                        let mark = line[lead..].find(char::is_whitespace).unwrap_or(line.len() - lead);
                        fill(&mut kinds, r.start + lead..r.start + lead + mark, Some(Kind::Marker));
                        continue;
                    }
                    _ => continue,
                };
                fill(&mut kinds, r, Some(k));
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some((stream, pieces)) = fence.take() {
                    code(text, &mut kinds, stream, &pieces);
                }
            }
            Event::Text(_) => {
                if let Some((_, pieces)) = &mut fence {
                    pieces.push(r);
                }
            }
            Event::Code(_) | Event::InlineMath(_) | Event::DisplayMath(_) => fill(&mut kinds, r, Some(Kind::Code)),
            Event::Html(_) | Event::InlineHtml(_) => fill(&mut kinds, r, Some(Kind::Html)),
            Event::Rule => fill(&mut kinds, r, Some(Kind::Rule)),
            Event::TaskListMarker(_) => fill(&mut kinds, r, Some(Kind::Task)),
            Event::FootnoteReference(_) => fill(&mut kinds, r, Some(Kind::Link)),
            // The break's bytes are the spaces and the line's end (`\r\n` or
            // `\n`); a backslash break starts at its `\`.
            Event::HardBreak if text.as_bytes().get(r.start) != Some(&b'\\') => {
                let spaces = text[r.clone()].find(['\r', '\n']).unwrap_or(r.len());
                if spaces > 0 {
                    breaks.push(r.start..r.start + spaces);
                }
            }
            _ => {}
        }
    }
    (kinds, breaks)
}

/// `ranges` (in order, none across a line's end) as `(line, bytes from the
/// line's start)`.
fn by_line(text: &str, ranges: &[Range<usize>]) -> Vec<(usize, Range<usize>)> {
    let bytes = text.as_bytes();
    let (mut line, mut start, mut at) = (0, 0, 0);
    let mut out = Vec::with_capacity(ranges.len());
    for r in ranges {
        let to = r.start.clamp(at, bytes.len());
        for (i, &b) in bytes[at..to].iter().enumerate() {
            if b == b'\n' {
                line += 1;
                start = at + i + 1;
            }
        }
        at = to;
        out.push((line, r.start.saturating_sub(start)..r.end.saturating_sub(start)));
    }
    out
}

/// A fence's content: the source bytes of its pieces (a quote's `>` left
/// out), read line by line by the fence's language, or `Code` when there is
/// none.
fn code(text: &str, kinds: &mut [Option<Kind>], stream: Option<ito_syntax::Stream>, pieces: &[Range<usize>]) {
    let Some(mut stream) = stream else {
        for r in pieces {
            fill(kinds, r.clone(), Some(Kind::Code));
        }
        return;
    };
    // The content as one text, and where each of its bytes is in the source.
    let mut src = Vec::new();
    let mut at = Vec::new();
    for r in pieces {
        let bytes = &text.as_bytes()[r.clone()];
        for (i, &b) in bytes.iter().enumerate() {
            if b == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
                continue;
            }
            src.push(b);
            at.push(r.start + i);
        }
    }
    // Bytes were copied whole, so this is the source's UTF-8 less `\r`s.
    let Ok(src) = String::from_utf8(src) else { return };
    for &i in &at {
        kinds[i] = None;
    }
    let mut start = 0;
    for line in src.split_inclusive('\n') {
        let body = line.strip_suffix('\n').unwrap_or(line);
        for (r, tok) in stream.line(body) {
            if r.start < r.end {
                let (s, e) = (at[start + r.start], at[start + r.end - 1] + 1);
                fill(kinds, s..e, Some(Kind::Syntax(tok)));
            }
        }
        start += line.len();
    }
}

/// `kinds` cut into lines and run-length coded.
fn lines(text: &str, kinds: &[Option<Kind>]) -> Vec<Runs> {
    let mut out = Vec::new();
    let mut start = 0;
    for line in text.split('\n') {
        let body = line.strip_suffix('\r').unwrap_or(line);
        let mut runs: Runs = Vec::new();
        for (i, k) in kinds[start..start + body.len()].iter().enumerate() {
            let Some(k) = *k else { continue };
            match runs.last_mut() {
                Some((r, last)) if *last == k && r.end == i => r.end = i + 1,
                _ => runs.push((i..i + 1, k)),
            }
        }
        out.push(runs);
        start += line.len() + 1;
    }
    out
}

/// The thread that reads the text. A job is the text at a version; while it
/// reads, newer jobs pile up and only the newest is read next.
pub struct Worker {
    tx: Sender<(u64, String)>,
    rx: Receiver<Tree>,
    asked: Option<u64>,
}

impl Worker {
    /// `wake` is called when a reading is ready (the window's repaint).
    pub fn new(wake: impl Fn() + Send + 'static) -> Worker {
        let (tx, jobs) = mpsc::channel::<(u64, String)>();
        let (done, rx) = mpsc::channel();
        let run = move || {
            while let Ok(mut job) = jobs.recv() {
                while let Ok(newer) = jobs.try_recv() {
                    job = newer;
                }
                if done.send(read(job.0, &job.1)).is_err() {
                    return;
                }
                wake();
            }
        };
        // Without the thread there are no colours, and the text is still there.
        let _ = std::thread::Builder::new().name("weft-structure".into()).spawn(run);
        Worker { tx, rx, asked: None }
    }

    /// Ask for a reading of `text` at `version`, unless it was asked already.
    pub fn ask(&mut self, version: u64, text: &str) {
        if self.asked == Some(version) {
            return;
        }
        self.asked = Some(version);
        let _ = self.tx.send((version, text.to_owned()));
    }

    /// The newest reading that came back since the last call.
    pub fn take(&self) -> Option<Tree> {
        self.rx.try_iter().last()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ito_syntax::Tok;

    /// The kinds of line `n` as `(text, kind)`.
    fn runs<'a>(text: &'a str, t: &Tree, n: usize) -> Vec<(&'a str, Kind)> {
        let line = text.split('\n').nth(n).unwrap();
        t.lines[n].iter().map(|(r, k)| (&line[r.clone()], *k)).collect()
    }

    #[test]
    fn headings_emphasis_and_code_are_coloured_inner_over_outer() {
        let text = "# Title *em*\n\nplain **bold** `x` ~~gone~~ [a](b)\n";
        let t = read(3, text);
        assert_eq!(t.version, 3);
        assert_eq!(t.headings.len(), 1);
        assert_eq!(t.headings[0].title, "Title em");
        assert_eq!(runs(text, &t, 0), [("# Title ", Kind::Heading), ("*em*", Kind::Emphasis)]);
        assert_eq!(runs(text, &t, 1), []);
        assert_eq!(
            runs(text, &t, 2),
            [("**bold**", Kind::Strong), ("`x`", Kind::Code), ("~~gone~~", Kind::Strike), ("[a](b)", Kind::Link)]
        );
        assert_eq!(t.lines.len(), 4, "a line after the last break");
    }

    #[test]
    fn a_list_paints_its_markers_and_task_boxes_only() {
        let text = "- one\n  1. two\n- [x] done\n";
        let t = read(1, text);
        assert_eq!(runs(text, &t, 0), [("-", Kind::Marker)]);
        assert_eq!(runs(text, &t, 1), [("1.", Kind::Marker)]);
        assert_eq!(runs(text, &t, 2), [("-", Kind::Marker), ("[x]", Kind::Task)]);
    }

    #[test]
    fn a_known_fence_is_read_by_its_language_and_an_unknown_one_is_code() {
        let text = "```rust\nfn main() {}\n```\n\n```nope\nabc\n```\n";
        let t = read(1, text);
        assert_eq!(runs(text, &t, 0), [("```rust", Kind::Fence)]);
        let body = runs(text, &t, 1);
        assert!(body.contains(&("fn", Kind::Syntax(Tok::Keyword))), "{body:?}");
        assert!(!body.iter().any(|(_, k)| *k == Kind::Fence), "the content is not the fence's: {body:?}");
        assert_eq!(runs(text, &t, 2), [("```", Kind::Fence)]);
        assert_eq!(runs(text, &t, 5), [("abc", Kind::Code)]);
    }

    #[test]
    fn crlf_lines_leave_the_cr_out_and_a_fence_reads_across_them() {
        let text = "# A\r\n\r\n```rust\r\nlet s = \"x\";\r\n```\r\n";
        let t = read(1, text);
        assert_eq!(runs(text, &t, 0), [("# A", Kind::Heading)]);
        let body = runs(text, &t, 3);
        assert!(body.contains(&("let", Kind::Syntax(Tok::Keyword))), "{body:?}");
        assert!(body.contains(&("\"x\"", Kind::Syntax(Tok::Str))), "{body:?}");
        for line in &t.lines {
            for (r, _) in line {
                assert!(r.start < r.end);
            }
        }
    }

    #[test]
    fn front_matter_is_not_a_heading() {
        let text = "---\ntitle: x\n---\n\n## Real\n";
        let t = read(1, text);
        assert_eq!(t.headings.len(), 1);
        assert_eq!(runs(text, &t, 1), [("title: x", Kind::Meta)]);
        assert_eq!(runs(text, &t, 4), [("## Real", Kind::Heading)]);
    }

    /// 8.10: Two spaces at the end of a paragraph's line are a hard break and
    /// are found, with CRLF line ends too and in a quote or a list; at a
    /// paragraph's end, in a fence, an indented block or a heading they do
    /// nothing and are not; a backslash break shows and is left alone.
    #[test]
    fn trailing_spaces_are_found_only_where_they_break_the_line() {
        // The breaks as (line, the marked text).
        fn found(text: &str) -> Vec<(usize, &str)> {
            let t = read(1, text);
            t.breaks.iter().map(|(n, r)| (*n, &text.split('\n').nth(*n).unwrap()[r.clone()])).collect()
        }
        assert_eq!(found("one  \ntwo\n"), [(0, "  ")]);
        assert_eq!(found("日本語   \nつづき"), [(0, "   ")], "three spaces after Japanese, no break at the end");
        assert_eq!(found("one \t\ntwo\n"), [(0, " \t")], "a tab counts as the parser counts it");
        assert_eq!(found("one \ntwo\n"), [], "one space is a soft break");
        assert_eq!(found("one\\\ntwo\n"), [], "a backslash break shows already");
        assert_eq!(found("para  \n\nnext  \n"), [], "at a paragraph's end the spaces do nothing");
        assert_eq!(found("last  "), [], "nor at the text's end");
        assert_eq!(found("```\ncode  \nmore\n```\n"), [], "not in a fence");
        assert_eq!(found("    code  \n    more\n"), [], "not in an indented block");
        assert_eq!(found("# Title  \ntext\n"), [], "not after a heading");
        assert_eq!(found("> quote  \n> more\n- item  \n  more\n"), [(0, "  "), (2, "  ")]);
        assert_eq!(found("a  \r\nb  \r\nc\r\n"), [(0, "  "), (1, "  ")], "CRLF: the `\\r` is not a space");

        let t = read(1, "x\n\na  \nb");
        assert_eq!(t.hard_break(2), Some(1..3));
        assert_eq!(t.hard_break(0), None);
        assert_eq!(t.hard_break(9), None);
    }

    #[test]
    fn the_worker_reads_the_newest_and_asks_once_per_version() {
        let mut w = Worker::new(|| {});
        w.ask(1, "a");
        w.ask(2, "# b");
        w.ask(2, "ignored: the same version");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut last = None;
        while last.as_ref().is_none_or(|t: &Tree| t.version < 2) {
            assert!(std::time::Instant::now() < deadline, "no reading came back");
            if let Some(t) = w.take() {
                last = Some(t);
            }
            std::thread::yield_now();
        }
        let t = last.unwrap();
        assert_eq!(t.version, 2);
        assert_eq!(t.headings.len(), 1, "read from the text of version 2");
        assert_eq!(t.chars, 3, "and counted from it");
        assert!(w.take().is_none());
    }

    /// 8.9: each reading counts the characters of its text, the line breaks
    /// left out, so the status bar shows the count without counting on a key.
    #[test]
    fn a_reading_counts_the_characters_of_its_text() {
        assert_eq!(read(1, "").chars, 0);
        assert_eq!(read(1, "# 見出し\r\n\r\n👍🏽 e\u{301}\r\n").chars, 8);
    }
}
