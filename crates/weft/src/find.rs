//! Find and replace. The bar is `ito-find`'s and the matching is `ito-match`'s;
//! this is what weft does with them: a thread that finds the matches of the
//! newest query in the newest text (off the UI thread, a result for an older
//! version is not shown), the highlights, stepping, and Replace / Replace all
//! as one undo step each.

use std::ops::Range;
use std::sync::mpsc::{self, Receiver, Sender};

use ito_find::{cmd, FindBar, Output, Status, Words};
use ito_textbuf::Selection;

use crate::engine::Editor;
use crate::App;

/// What a search was asked about: the query, the text, and the part of the
/// text searched (`None`: all of it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Key {
    bar: u64,
    text: u64,
    within: Option<Range<usize>>,
}

struct Job {
    key: Key,
    bar: FindBar,
    text: String,
}

/// The matches of a search, or why the query is not one.
struct Found {
    key: Key,
    ranges: Result<Vec<Range<usize>>, String>,
}

/// The search of one document: the thread, the newest result, and the part of
/// the text "in selection" is limited to.
pub struct DocFinder {
    tx: Sender<Job>,
    rx: Receiver<Found>,
    asked: Option<Key>,
    found: Option<Found>,
    /// The selection that was there when "in selection" went on. Stepping moves
    /// the selection, so it is kept here.
    scope: Option<Range<usize>>,
}

impl DocFinder {
    /// `wake` is called when a result is ready (the window's repaint).
    pub fn new(wake: impl Fn() + Send + 'static) -> Self {
        let (tx, jobs) = mpsc::channel::<Job>();
        let (done, rx) = mpsc::channel();
        let run = move || {
            while let Ok(mut job) = jobs.recv() {
                while let Ok(newer) = jobs.try_recv() {
                    job = newer;
                }
                let ranges = job.bar.ranges(&job.text, job.key.within.clone());
                if done.send(Found { key: job.key, ranges }).is_err() {
                    return;
                }
                wake();
            }
        };
        // Without the thread nothing is highlighted; stepping still finds.
        let _ = std::thread::Builder::new().name("weft-find".into()).spawn(run);
        Self { tx, rx, asked: None, found: None, scope: None }
    }

    /// The matches to paint: those of `key`, if that is what was found.
    pub fn marks(&self, key: &Key) -> &[Range<usize>] {
        match &self.found {
            Some(Found { key: k, ranges: Ok(r) }) if k == key => r,
            _ => &[],
        }
    }
}

/// What a search of `ed`'s text by `bar` is called, given `finder`'s scope.
pub fn key(bar: &FindBar, ed: &Editor, finder: &DocFinder) -> Key {
    Key { bar: bar.version(), text: ed.version(), within: bar.in_selection.then(|| finder.scope.clone()).flatten() }
}

/// The words on the bar, from the language table.
pub fn words(t: &ito_i18n::Table) -> Words<'_> {
    Words {
        find_hint: t.tr("find.find_hint"),
        replace_hint: t.tr("find.replace_hint"),
        regex: t.tr("find.regex"),
        match_case: t.tr("find.match_case"),
        whole_word: t.tr("find.whole_word"),
        in_selection: t.tr("find.in_selection"),
        next: t.tr("find.next"),
        prev: t.tr("find.prev"),
        replace: t.tr("find.replace"),
        replace_all: t.tr("find.replace_all"),
        close: t.tr("find.close"),
        toggle_replace: t.tr("find.toggle_replace"),
        no_results: t.tr("find.no_results"),
        searching: t.tr("find.searching"),
    }
}

/// The 1-based index of the match the selection is on (or the caret is in).
fn current(ranges: &[Range<usize>], ed: &Editor) -> Option<usize> {
    let sel = ed.selection().range();
    ranges.iter().position(|r| *r == sel || (sel.is_empty() && r.start <= sel.start && sel.start <= r.end)).map(|i| i + 1)
}

impl App {
    /// Once a frame while the bar is open: keep the scope, ask for the search
    /// the text and the query call for, take what came back, and tell the bar.
    pub(crate) fn find_sync(&mut self) {
        if !self.find.open {
            return;
        }
        let Some(ed) = &self.doc.editor else { return };
        let f = &mut self.doc.finder;
        while let Ok(found) = f.rx.try_recv() {
            f.found = Some(found);
        }
        if !self.find.in_selection {
            f.scope = None;
        } else if f.scope.is_none() && !ed.selection().is_empty() {
            f.scope = Some(ed.selection().range());
        }
        if self.find.find.is_empty() {
            return;
        }
        let key = key(&self.find, ed, f);
        let have = f.found.as_ref().is_some_and(|x| x.key == key);
        if !have && f.asked.as_ref() != Some(&key) {
            f.asked = Some(key.clone());
            let _ = f.tx.send(Job { key: key.clone(), bar: self.find.clone(), text: ed.text().to_owned() });
        }
        if let Some(found) = f.found.as_ref().filter(|x| x.key == key) {
            let result = match &found.ranges {
                Ok(r) => Ok(Status::Found { current: current(r, ed), total: r.len() }),
                Err(e) => Err(e.clone()),
            };
            self.find.set_result(self.find.version(), result);
        }
    }

    /// The bar under the tabs, while it is open on a document. It is drawn
    /// before the document is, so what it says of the search is the one
    /// `find_sync` took the frame before.
    pub(crate) fn find_bar(&mut self, ui: &mut egui::Ui) {
        if !self.find.open || self.settings_open || self.doc.editor.is_none() {
            self.find_focus = false;
            return;
        }
        let words = words(&self.table);
        let find = &mut self.find;
        let look = ito_prefs::Look { c: ito_theme::colors(), q: "" };
        let mut out = Output::default();
        egui::Panel::top("find")
            .frame(egui::Frame::NONE.fill(look.c.bg).inner_margin(egui::Margin::symmetric(8, 6)))
            .show(ui, |ui| out = find.show(ui, look, &words));
        // The bar has the focus when one of its widgets does; the document does
        // not type then.
        self.find_focus = self.find.open && ui.ctx().memory(|m| m.focused().is_some());
        self.find_output(out);
    }

    /// The matches now: the thread's if it has them, else found here (a step
    /// or a replace is asked for, and waits for its answer).
    fn find_ranges(&mut self) -> Result<Vec<Range<usize>>, String> {
        let Some(ed) = &self.doc.editor else { return Ok(Vec::new()) };
        let f = &mut self.doc.finder;
        let key = key(&self.find, ed, f);
        if let Some(found) = f.found.as_ref().filter(|x| x.key == key) {
            return found.ranges.clone();
        }
        let ranges = self.find.ranges(ed.text(), key.within.clone());
        f.found = Some(Found { key, ranges: ranges.clone() });
        ranges
    }

    /// A find command by name: open the bar, step, replace all.
    pub(crate) fn find_command(&mut self, ctx: &egui::Context, name: &str) {
        if self.doc.editor.is_none() {
            return;
        }
        match name {
            cmd::OPEN | cmd::REPLACE_OPEN => {
                // The selected word is the query, unless the bar is being typed in.
                let seed = if self.find_focus { None } else { self.doc.editor.as_ref().and_then(Editor::selected_text) };
                self.find.command(name, seed.as_deref());
                // The field takes the focus when the bar is drawn, next frame.
                ctx.request_repaint();
            }
            // Nothing to step through or replace yet: the bar opens for the query.
            _ if !self.find.open || self.find.find.is_empty() => {
                let open = if name == cmd::REPLACE_ALL { cmd::REPLACE_OPEN } else { cmd::OPEN };
                self.find.command(open, None);
                ctx.request_repaint();
            }
            cmd::NEXT => self.find_step(true),
            cmd::PREV => self.find_step(false),
            cmd::REPLACE_ALL => self.find_replace_all(),
            _ => {}
        }
    }

    /// What the bar's buttons and keys asked this frame.
    pub(crate) fn find_output(&mut self, out: Output) {
        if out.closed {
            self.find_focus = false;
            return;
        }
        if out.next {
            self.find_step(true);
        } else if out.prev {
            self.find_step(false);
        }
        if out.replace_one {
            self.find_replace_one();
        }
        if out.replace_all {
            self.find_replace_all();
        }
    }

    /// Select the next (or previous) match from the selection, wrapping round
    /// at the ends of the text.
    pub(crate) fn find_step(&mut self, forward: bool) {
        let ranges = match self.find_ranges() {
            Ok(r) => r,
            Err(e) => return self.find_said(e),
        };
        let Some(ed) = &mut self.doc.editor else { return };
        let sel = ed.selection().range();
        let i = if forward { ito_match::nth_after(&ranges, sel.end) } else { ito_match::nth_before(&ranges, sel.start) };
        let Some(i) = i else {
            let text = self.table.tr("find.no_results").to_owned();
            return self.find_said(text);
        };
        let r = ranges[i].clone();
        ed.set_caret(r.start, false);
        ed.set_caret(r.end, true);
        self.doc.view.follow = true;
        self.find.status = Status::Found { current: Some(i + 1), total: ranges.len() };
        let said = format!("{} / {}", i + 1, ranges.len());
        self.find_said(said);
    }

    /// Replace the match the selection is on and go on to the next; when the
    /// selection is not on one, select the next first.
    pub(crate) fn find_replace_one(&mut self) {
        let subst = match self.find.subst() {
            Ok(s) => s,
            Err(e) => return self.find_said(e),
        };
        let ranges = match self.find_ranges() {
            Ok(r) => r,
            Err(e) => return self.find_said(e),
        };
        let Some(ed) = &mut self.doc.editor else { return };
        let sel = ed.selection().range();
        if !ranges.contains(&sel) {
            return self.find_step(true);
        }
        // The edit is made by the substitution, so `$1` is what the match had.
        let Some(edit) = subst.edits_in(ed.text(), sel.clone()).into_iter().find(|(r, _)| *r == sel) else { return };
        let end = edit.0.start + edit.1.len();
        let delta = edit.1.len() as isize - edit.0.len() as isize;
        if !ed.replace(&[edit], Selection::caret(end)) {
            return self.find_locked();
        }
        if let Some(r) = &mut self.doc.finder.scope {
            r.end = (r.end as isize + delta).max(r.start as isize) as usize;
        }
        self.doc.view.follow = true;
        self.find_step(true);
    }

    /// Replace every match (in the selection, when "in selection" is on) as one
    /// undo step.
    pub(crate) fn find_replace_all(&mut self) {
        let subst = match self.find.subst() {
            Ok(s) => s,
            Err(e) => return self.find_said(e),
        };
        let Some(ed) = &mut self.doc.editor else { return };
        let scope = if self.find.in_selection { self.doc.finder.scope.clone() } else { None };
        let edits = match &scope {
            Some(r) => subst.edits_in(ed.text(), r.clone()),
            None => subst.edits(ed.text()),
        };
        let Some(first) = edits.first().map(|(r, _)| r.start) else {
            let text = self.table.tr("find.no_results").to_owned();
            return self.find_said(text);
        };
        let delta: isize = edits.iter().map(|(r, t)| t.len() as isize - r.len() as isize).sum();
        if !ed.replace(&edits, Selection::caret(first)) {
            return self.find_locked();
        }
        if let Some(r) = &mut self.doc.finder.scope {
            r.end = (r.end as isize + delta).max(r.start as isize) as usize;
        }
        self.doc.view.follow = true;
        let text = self.table.trf("find.replaced", &[&edits.len().to_string()]);
        self.find_said(text.clone());
        self.note = Some(text);
    }

    /// The document cannot be changed.
    fn find_locked(&mut self) {
        let text = self.table.tr("find.locked").to_owned();
        self.find_said(text.clone());
        self.note = Some(text);
    }

    /// `text` for the screen reader.
    fn find_said(&mut self, text: String) {
        self.announce(text);
    }

    /// What `<State:>` says of the search.
    pub(crate) fn find_report(&self) -> String {
        let words = words(&self.table);
        let mut out = format!("find: {}\n", if self.find.open { "open" } else { "closed" });
        if self.find.open {
            out.push_str(&format!("find_query: {}\nfind_status: {}\n", self.find.find, self.find.label(&words)));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(text: &str) -> App {
        let mut app = App::for_tests();
        app.doc.editor = Some(Editor::new(text));
        app
    }

    fn text(app: &App) -> &str {
        app.doc.editor.as_ref().unwrap().text()
    }

    fn select(app: &mut App, r: Range<usize>) {
        let ed = app.doc.editor.as_mut().unwrap();
        ed.set_caret(r.start, false);
        ed.set_caret(r.end, true);
    }

    fn query(app: &mut App, find: &str, replace: &str) {
        app.find.open_replace(Some(find));
        app.find.replace = replace.to_owned();
    }

    /// 14.1: Replace all is one undo step; `$1` is what the group matched, and
    /// the line endings of the text stay as they were.
    #[test]
    fn replace_all_is_one_undo_step() {
        let before = "a1\r\nb2\r\nc3\r\n";
        let mut app = app(before);
        query(&mut app, r"([a-c])(\d)", "$2$1");
        app.find.regex = true;
        app.find_replace_all();
        assert_eq!(text(&app), "1a\r\n2b\r\n3c\r\n");
        assert_eq!(app.doc.editor.as_ref().unwrap().selection(), Selection::caret(0));
        assert!(app.doc.editor.as_mut().unwrap().undo());
        assert_eq!(text(&app), before, "one undo restores all three");
    }

    /// 14.2: With "in selection", only the selected text is replaced.
    #[test]
    fn replace_all_in_the_selection_only() {
        let mut app = app("x x x x");
        query(&mut app, "x", "yy");
        app.find.in_selection = true;
        select(&mut app, 2..5); // the middle two
        app.find_sync();
        app.find_replace_all();
        assert_eq!(text(&app), "x yy yy x");
    }

    /// 14.3: A file that cannot be written is not replaced in, and says so.
    #[test]
    fn replace_all_refuses_a_locked_file() {
        let p = std::env::temp_dir().join(format!("weft-find-locked-{}.md", std::process::id()));
        std::fs::write(&p, [b'a', 0xff, b'a']).unwrap();
        let mut app = App::for_tests();
        app.doc.editor = Some(Editor::open(&p).unwrap());
        query(&mut app, "a", "b");
        let before = text(&app).to_owned();
        app.find_replace_all();
        assert_eq!(text(&app), before);
        assert_eq!(app.note.as_deref(), Some("This file cannot be changed"));
        let _ = std::fs::remove_file(&p);
    }

    /// 14.4: Next and previous wrap round at the ends.
    #[test]
    fn stepping_wraps() {
        let mut app = app("ab ab ab");
        query(&mut app, "ab", "");
        let sel = |a: &App| a.doc.editor.as_ref().unwrap().selection().range();
        app.find_step(true);
        assert_eq!(sel(&app), 0..2);
        app.find_step(true);
        assert_eq!(sel(&app), 3..5);
        app.find_step(true);
        app.find_step(true);
        assert_eq!(sel(&app), 0..2, "wrapped to the first");
        app.find_step(false);
        assert_eq!(sel(&app), 6..8, "back wraps to the last");
    }

    /// 14.5: Replace puts the replacement in for the match the selection is
    /// on and moves to the next.
    #[test]
    fn replace_one_goes_on() {
        let mut app = app("ab ab");
        query(&mut app, "ab", "X");
        app.find_step(true);
        app.find_replace_one();
        assert_eq!(text(&app), "X ab");
        assert_eq!(app.doc.editor.as_ref().unwrap().selection().range(), 2..4);
    }

    /// 14.6: A result for an older query is not the one shown.
    #[test]
    fn an_old_result_is_dropped() {
        let mut app = app("ab ab");
        query(&mut app, "ab", "");
        let old = app.find.version();
        app.find.find = "b".into();
        let mut b = app.find.clone();
        b.open_find(Some("zz")); // a new version
        app.find = b;
        assert!(!app.find.set_result(old, Ok(Status::Found { current: None, total: 2 })));
    }

    /// 14.7: A query that is not a regex says why and replaces nothing.
    #[test]
    fn a_bad_regex_replaces_nothing() {
        let mut app = app("a(b");
        query(&mut app, "(", "x");
        app.find.regex = true;
        app.find_replace_all();
        assert_eq!(text(&app), "a(b");
    }
}
