//! The document tabs: one per open file, in the order `ito-tabs` keeps them.
//!
//! The front tab's document is `App::doc`; the others are parked in
//! `App::parked`, each with its own editor, view and structure worker, and
//! come forward by swapping places with it. Every tab operation changes
//! `App::tabs` first and then [`App::reconcile`] makes `doc` and `parked`
//! agree with it, so a close, a switch and an eviction share one path.

use std::path::{Path, PathBuf};

use ito_tabs::Op;

use crate::{engine, App, Doc};

impl Doc {
    /// The file this document is, as its tab names it.
    pub(crate) fn path(&self) -> Option<PathBuf> {
        self.editor.as_ref().and_then(engine::Editor::path).map(Path::to_path_buf)
    }

    /// The document has changes not yet saved.
    pub(crate) fn dirty(&self) -> bool {
        self.editor.as_ref().is_some_and(engine::Editor::is_dirty)
    }
}

/// `path` as a tab names it: absolute, so `a.md` and `./a.md` are one tab.
pub(crate) fn tab_id(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

impl App {
    /// A document with nothing open, whose worker wakes this window.
    pub(crate) fn new_doc(&self) -> Doc {
        let ctx = self.ctx.clone();
        Doc::new(move || ctx.request_repaint())
    }

    /// A document of the file `path`: a big one is read by line (`big`), any other is loaded.
    fn read_doc(&self, path: &Path) -> Result<Doc, String> {
        let mut doc = self.new_doc();
        if crate::big::is_big(path) {
            doc.big = Some(crate::big::Big::open(path).map_err(|e| format!("{}: {e}", path.display()))?);
            doc.editor = Some(engine::Editor::open_big(path));
        } else {
            doc.editor = Some(engine::Editor::open(path)?);
            doc.view.folds.restore(self.kept_folds(path));
        }
        Ok(doc)
    }

    /// The document of the tab `path` (the front one or a parked one).
    pub(crate) fn doc_mut(&mut self, path: &Path) -> Option<&mut Doc> {
        if self.doc.path().as_deref() == Some(path) {
            return Some(&mut self.doc);
        }
        self.parked.iter_mut().find(|(p, _)| p == path).map(|(_, d)| d)
    }

    /// The document of the tab `path`, to look at.
    pub(crate) fn doc_ref(&self, path: &Path) -> Option<&Doc> {
        if self.doc.path().as_deref() == Some(path) {
            return Some(&self.doc);
        }
        self.parked.iter().find(|(p, _)| p == path).map(|(_, d)| d)
    }

    /// Every open document, the front one first.
    pub(crate) fn docs(&self) -> impl Iterator<Item = &Doc> {
        std::iter::once(&self.doc).chain(self.parked.iter().map(|(_, d)| d)).filter(|d| d.editor.is_some())
    }

    /// Some open document has changes not yet saved.
    pub(crate) fn any_dirty(&self) -> bool {
        self.docs().any(Doc::dirty)
    }

    /// Saves of any document still with the file thread.
    pub(crate) fn saving(&self) -> usize {
        self.docs().map(|d| d.saving).sum()
    }

    /// The paths of the documents with changes not saved, the front one first.
    pub(crate) fn dirty_paths(&self) -> Vec<PathBuf> {
        self.docs().filter(|d| d.dirty()).filter_map(Doc::path).collect()
    }

    /// Put `doc` in front; the one that was there stays as a parked tab when
    /// its tab is still open, and is let go of when it was closed.
    fn install_front(&mut self, doc: Doc) {
        let old = std::mem::replace(&mut self.doc, doc);
        if let Some(p) = old.path().filter(|p| self.tabs.position(p).is_some()) {
            self.parked.push((p, old));
        }
    }

    /// Make the documents agree with `tabs`: the closed ones go, and the
    /// front tab's document is the one in front. Called after every change to
    /// the tabs.
    pub(crate) fn reconcile(&mut self) {
        self.parked.retain(|(p, _)| self.tabs.position(p).is_some());
        let want = self.tabs.front().cloned();
        if want != self.doc.path() {
            let next = want
                .and_then(|w| self.parked.iter().position(|(p, _)| *p == w))
                .map(|i| self.parked.remove(i).1)
                .unwrap_or_else(|| self.new_doc());
            self.install_front(next);
            // A chord half typed in the tab left is not the next tab's.
            self.key_pending.clear();
            self.doc.view.follow = true;
            let name = self.doc.path().and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()));
            if let Some(name) = name {
                self.announce(name);
            }
        }
        self.tabs.save(&self.tab_writer);
    }

    /// The tabs kept at `path` by the last run come back: the files that still
    /// read, in their order, with the one that was in front in front again.
    pub(crate) fn restore_tabs(&mut self, path: &Path) {
        let kept = ito_tabs::Tabs::<PathBuf>::load(path, self.tabs.limit());
        let front = kept.front().cloned();
        for id in kept.items() {
            // `Editor::open` makes a new document of a path that is not there (`weft new.md`); a tab of the last run is not that.
            if !id.is_file() {
                continue;
            }
            let Ok(doc) = self.read_doc(id) else { continue };
            self.tabs.open(id.clone());
            self.parked.push((id.clone(), doc));
        }
        if let Some(i) = front.and_then(|f| self.tabs.position(&f)) {
            self.tabs.apply(Op::Goto(i));
        }
        self.reconcile();
    }

    /// Open `path` as a tab, or bring the tab it already has forward. One
    /// that cannot be read opens no tab, and one that is not UTF-8 opens
    /// read-only; both are told of under the bell. When the tabs are full and
    /// the one to make room for has changes not saved, nothing opens.
    pub(crate) fn open(&mut self, path: &Path) {
        let id = tab_id(path);
        if self.tabs.position(&id).is_some() {
            self.tabs.open(id);
            self.reconcile();
            return;
        }
        let name = id.display().to_string();
        if self.tabs.len() >= self.tabs.limit() {
            // The leftmost tab goes (a new one opens beside the front one, so never at 0).
            if let Some(full) = self.tabs.items().first().cloned().filter(|v| self.doc_mut(v).is_some_and(|d| d.dirty())) {
                let title = self.table.trf("tab.full", &[&name, &full.display().to_string()]);
                self.outcome(ito_notices::Kind::Warn, title, Some(full));
                return;
            }
        }
        match self.read_doc(&id) {
            Ok(doc) => {
                let ed = doc.editor.as_ref().expect("read_doc opens an editor");
                if ed.locked() == Some(engine::Locked::Big) {
                    let title = self.table.trf("file.big", &[&name]);
                    self.outcome(ito_notices::Kind::Info, title, Some(id.clone()));
                } else if ed.locked().is_some() {
                    let title = self.table.trf("file.locked", &[&name]);
                    self.outcome(ito_notices::Kind::Warn, title, Some(id.clone()));
                }
                self.tabs.open_evicting(id);
                self.install_front(doc);
                self.reconcile();
            }
            Err(e) => {
                let title = self.table.trf("file.open_failed", &[&name, &e]);
                self.outcome(ito_notices::Kind::Err, title, None);
            }
        }
    }

    /// What the person asked of the tabs (a key, the bar, the palette). Closing
    /// a tab with changes not saved asks first.
    pub(crate) fn tab_op(&mut self, op: Op) {
        let at = match op {
            Op::Close => Some(self.tabs.front_index()),
            Op::CloseAt(i) => Some(i),
            _ => None,
        };
        match at {
            Some(i) => {
                let Some(path) = self.tabs.items().get(i).cloned() else { return };
                if self.doc_mut(&path).is_some_and(|d| d.dirty()) {
                    self.help.close();
                    self.palette = None;
                    self.bell_open = false;
                    self.announce(self.close_spoken(&path));
                    self.closing = Some(path);
                } else {
                    self.close_tab(&path);
                }
            }
            None => {
                self.tabs.apply(op);
                self.reconcile();
            }
        }
    }

    /// Close the tab `path`, whatever it holds.
    pub(crate) fn close_tab(&mut self, path: &Path) {
        if let Some(i) = self.tabs.position(&path.to_path_buf()) {
            self.tabs.apply(Op::CloseAt(i));
            self.reconcile();
        }
    }

    /// Bring the tab `path` forward when it is open.
    pub(crate) fn show_tab(&mut self, path: &Path) {
        let id = tab_id(path);
        if self.tabs.position(&id).is_some() {
            self.tabs.open(id);
            self.reconcile();
        }
    }

    /// The question before a tab with changes closes, as the screen reader says it.
    fn close_spoken(&self, path: &Path) -> String {
        let t = &self.table;
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let body = t.trf("quit.body", &[&name]);
        t.trf("quit.spoken", &[t.tr("quit.title"), &body, t.tr("tab.save"), t.tr("tab.discard"), t.tr("quit.cancel")])
    }

    /// The close question's answer: 0 stay, 1 close without saving, 2 save and close.
    fn answer_close(&mut self, pick: usize) {
        let Some(path) = self.closing.take() else { return };
        match pick {
            0 => {}
            1 => self.close_tab(&path),
            _ => {
                // Closes when the save lands (and the tab is still not changed).
                self.close_when_saved.push(path.clone());
                self.save_at(&path);
            }
        }
    }

    /// The close question's keys, read before anything else sees them: Esc
    /// stays, `n` closes without saving, Enter (or `y`) saves and closes.
    pub(crate) fn closing_keys(&mut self, ctx: &egui::Context) {
        use egui::{Key as K, Modifiers as M};
        let pick = ctx.input_mut(|i| {
            if i.consume_key(M::NONE, K::Escape) {
                Some(0)
            } else if i.consume_key(M::NONE, K::N) {
                Some(1)
            } else if i.consume_key(M::NONE, K::Enter) || i.consume_key(M::NONE, K::Y) {
                Some(2)
            } else {
                None
            }
        });
        if let Some(p) = pick {
            self.answer_close(p);
        }
    }

    /// The question before a tab with changes closes, over everything; a
    /// click is its key.
    pub(crate) fn closing_question(&mut self, ctx: &egui::Context) {
        let Some(path) = self.closing.clone() else { return };
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let body = [self.table.trf("quit.body", &[&name])];
        let buttons = [
            format!("{}  Esc", self.table.tr("quit.cancel")),
            format!("{}  N", self.table.tr("tab.discard")),
            format!("{}  Enter", self.table.tr("tab.save")),
        ];
        let q = ito_confirm::Question {
            title: self.table.tr("quit.title"),
            body: &body,
            buttons: &buttons,
            primary: Some(2),
            ..Default::default()
        };
        if let Some(i) = ito_confirm::show(ctx, &q, &crate::confirm_style()) {
            self.answer_close(i);
        }
    }

    /// The tab bar, when `[tabs] show_bar` says so; what it asks is done.
    pub(crate) fn tab_bar(&mut self, ui: &mut egui::Ui) {
        if !self.prefs.tabs.show_bar.visible(self.tabs.len()) {
            return;
        }
        let h = ito_tabs::HEIGHT * ito_theme::text_zoom(ui.ctx());
        let items: Vec<PathBuf> = self.tabs.items().to_vec();
        let labels = ito_tabs::labels(&items);
        let tips: Vec<String> = items.iter().map(|p| p.display().to_string()).collect();
        let unsaved: Vec<bool> = items.iter().map(|p| self.doc_ref(p).is_some_and(Doc::dirty)).collect();
        let front = self.tabs.front_index();
        let mut asked = None;
        egui::Panel::top("tabs").exact_size(h).frame(egui::Frame::NONE).show(ui, |ui| {
            let views: Vec<ito_tabs::TabView<'_>> =
                (0..items.len()).map(|i| ito_tabs::TabView { label: &labels[i], tooltip: &tips[i], unsaved: unsaved[i] }).collect();
            asked = ito_tabs::bar(ui, &views, front);
        });
        if let Some(op) = asked {
            self.tab_op(op);
        }
    }

    /// Ctrl+S on the tab `path`: what the document is now goes to the file thread.
    pub(crate) fn save_at(&mut self, path: &Path) {
        let Some(job) = self.doc_mut(path).and_then(|d| d.editor.as_ref().map(engine::Editor::save_job)) else { return };
        let name = path.display().to_string();
        match job {
            Ok(job) => {
                if self.send_file(crate::FileJob::Save(job)) {
                    if let Some(d) = self.doc_mut(path) {
                        d.saving += 1;
                    }
                }
            }
            Err(engine::NoSave::NoPath) => {
                let title = self.table.tr("file.no_path").to_owned();
                self.outcome(ito_notices::Kind::Warn, title, None);
            }
            Err(engine::NoSave::Locked(_)) => {
                let title = self.table.trf("file.locked", &[&name]);
                self.outcome(ito_notices::Kind::Warn, title, Some(path.to_path_buf()));
            }
        }
    }

    /// Save every document with changes (what "save and quit" does).
    pub(crate) fn save_all(&mut self) {
        for p in self.dirty_paths() {
            self.save_at(&p);
        }
        // A document with no file has nowhere to go: `save` tells so under the bell.
        if self.doc.dirty() && self.doc.path().is_none() {
            self.save();
        }
    }

    /// What the quit question says: the file with changes not saved, and how
    /// many others there are.
    pub(crate) fn quit_body(&self) -> String {
        let mut names: Vec<String> =
            self.dirty_paths().iter().filter_map(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).collect();
        if self.doc.dirty() && self.doc.path().is_none() {
            names.insert(0, self.table.tr("tab.untitled").to_owned());
        }
        match names.as_slice() {
            [] => self.table.trf("quit.body", &[""]),
            [one] => self.table.trf("quit.body", &[one]),
            [first, rest @ ..] => self.table.trf("quit.body_many", &[first, &rest.len().to_string()]),
        }
    }

    /// The tabs for `<State:…>`: their file names in order, `*` after those
    /// with changes not saved, and the index of the front one.
    pub(crate) fn tabs_report(&self) -> String {
        let list: Vec<String> = self
            .tabs
            .items()
            .iter()
            .map(|p| {
                let mark = if self.doc_ref(p).is_some_and(Doc::dirty) { "*" } else { "" };
                format!("{}{mark}", p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
            })
            .collect();
        format!("tabs: {}\nfront_tab: {}\n", list.join(", "), self.tabs.front_index() + 1)
    }
}

#[cfg(test)]
mod tests;
