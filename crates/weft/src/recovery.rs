//! Autosave for crash recovery: what is typed and not yet saved is copied to
//! `recovery.toml` in the state folder a moment after each change (on the
//! writer's thread, the newest winning). The file never replaces the user's
//! file. A clean exit empties it; a run that finds it non-empty asks whether
//! to bring the text back (one undo step over the file's text, still unsaved).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::App;

/// How long typing waits before it is copied.
const EVERY: Duration = Duration::from_secs(2);

/// An unsaved document: the file, and the text the window held.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct Entry {
    pub path: PathBuf,
    pub text: String,
}

#[derive(Serialize, Deserialize, Default)]
struct File {
    #[serde(default)]
    entries: Vec<Entry>,
}

pub(crate) fn state_path() -> Option<PathBuf> {
    ito_common::state_dir("weft").map(|d| d.join("recovery.toml"))
}

/// What a crashed run left that still differs from its file (a file that is
/// gone, or already holds the text, is not worth asking about).
pub(crate) fn read() -> Vec<Entry> {
    let Some(file) = state_path().and_then(|p| ito_common::kept::read::<File>(&p)) else { return Vec::new() };
    file.entries
        .into_iter()
        .filter(|e| match std::fs::read_to_string(&e.path) {
            Ok(disk) => disk.strip_prefix('\u{feff}').unwrap_or(&disk) != e.text,
            Err(_) => false,
        })
        .collect()
}

impl App {
    /// One frame: copy the unsaved documents when they changed since the last
    /// copy (at most every [`EVERY`]). Nothing is copied while the restore
    /// question is up: the file holds what it is about.
    pub(crate) fn take_recovery(&mut self, ctx: &egui::Context) {
        if !self.recover.is_empty() {
            return;
        }
        let marks: Vec<(PathBuf, u64)> =
            self.docs().filter(|d| d.dirty()).filter_map(|d| Some((d.path()?, d.editor.as_ref()?.version()))).collect();
        if marks == self.recovery_marks {
            return;
        }
        let since = self.recovery_at.elapsed();
        if since < EVERY {
            ctx.request_repaint_after(EVERY - since);
            return;
        }
        let entries = marks
            .iter()
            .filter_map(|(p, _)| Some(Entry { path: p.clone(), text: self.doc_ref(p)?.editor.as_ref()?.text().to_owned() }))
            .collect();
        self.recovery_writer.save_toml(&File { entries });
        self.recovery_marks = marks;
        self.recovery_at = Instant::now();
    }

    /// A clean exit: nothing is left to recover.
    pub(crate) fn clear_recovery(&mut self) {
        if self.recover.is_empty() {
            self.recovery_writer.save_toml(&File::default());
        }
    }

    /// The answer: 0 leave it, 1 bring the text back.
    pub(crate) fn answer_recover(&mut self, pick: usize) {
        let entries = std::mem::take(&mut self.recover);
        if pick == 1 {
            for e in entries {
                self.open(&e.path);
                let id = crate::tabs::tab_id(&e.path);
                if let Some(ed) = self.doc_mut(&id).and_then(|d| d.editor.as_mut()) {
                    ed.put_text(&e.text, false);
                }
            }
        }
        // The copies are the new run's to make now.
        self.recovery_marks.clear();
        self.recovery_at = Instant::now() - EVERY;
        if pick == 0 {
            self.recovery_writer.save_toml(&File::default());
        }
    }

    pub(crate) fn recover_open(&self) -> bool {
        !self.recover.is_empty()
    }

    /// Esc leaves it, Enter (or `y`) brings the text back.
    pub(crate) fn recover_keys(&mut self, ctx: &egui::Context) {
        use egui::{Key as K, Modifiers as M};
        if !self.recover_open() {
            return;
        }
        let pick = ctx.input_mut(|i| {
            if i.consume_key(M::NONE, K::Escape) {
                Some(0)
            } else if i.consume_key(M::NONE, K::Enter) || i.consume_key(M::NONE, K::Y) {
                Some(1)
            } else {
                None
            }
        });
        if let Some(p) = pick {
            self.answer_recover(p);
        }
    }

    pub(crate) fn recover_question(&mut self, ctx: &egui::Context) {
        if !self.recover_open() {
            return;
        }
        let t = &self.table;
        let mut body = vec![t.tr("recover.body").to_owned()];
        for e in self.recover.iter().take(8) {
            body.push(e.path.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned()));
        }
        if self.recover.len() > 8 {
            body.push(t.trf("recover.more", &[&(self.recover.len() - 8).to_string()]));
        }
        let buttons = [format!("{}  Esc", t.tr("recover.leave")), format!("{}  Enter", t.tr("recover.restore"))];
        let q =
            ito_confirm::Question { title: t.tr("recover.title"), body: &body, buttons: &buttons, primary: Some(1), ..Default::default() };
        if let Some(i) = ito_confirm::show(ctx, &q, &crate::confirm_style()) {
            self.answer_recover(i);
        }
    }
}
