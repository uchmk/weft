//! One window per user: with `open_from_outside = new_tab`, a second
//! `weft FILE` hands the file to the window already running (as a tab) and
//! goes away. The door itself is `ito-ipc`'s; this is what weft asks of it and
//! does with what arrives.

use std::path::Path;
use std::sync::{Arc, OnceLock};

use ito_ipc::single::{self, FileArg, OpenRequest, Outcome, Owner};

use crate::{tabs::tab_id, App, Goto};

/// Names another door than the user's own (a second instance for trying
/// something, and the tests).
const ENV: &str = "WEFT_DOOR";

/// Bumped when [`OpenRequest`] changes shape.
const PROTO: u32 = 1;

/// What `weft [+LINE[:COL]] [FILE]` asks of the running window. A launch
/// without a file asks only to come forward.
pub(crate) fn request(file: Option<&Path>, goto: Option<Goto>, more: &[std::path::PathBuf], cwd: &Path) -> OpenRequest {
    // The others first, so the file with the caret asked for ends in front.
    let mut files: Vec<FileArg> = more.iter().map(|f| FileArg { path: cwd.join(f), line: None, col: None }).collect();
    files.extend(file.map(|f| FileArg {
        path: cwd.join(f), // an absolute path replaces cwd
        line: goto.and_then(|g| u32::try_from(g.line).ok()),
        col: goto.and_then(|g| u32::try_from(g.col).ok()),
    }));
    OpenRequest { files, cwd: Some(cwd.to_path_buf()) }
}

/// What taking the door came to.
pub(crate) enum Door {
    /// This process owns the door.
    Owner(Owner<OpenRequest>),
    /// The running window has the request.
    Forwarded,
    /// The door could not be made; weft runs as a window of its own.
    Closed,
}

/// Take the door, or hand `req` to the window that has it. `ctx` is filled by
/// the window once it exists; a request that arrives before is read on the
/// first frame, so no repaint is lost.
pub(crate) fn acquire(req: &OpenRequest, ctx: Arc<OnceLock<egui::Context>>) -> Door {
    let wake = move || {
        if let Some(c) = ctx.get() {
            c.request_repaint();
        }
    };
    match single::acquire("weft", ENV, PROTO, req, wake) {
        Ok(Outcome::Owner(o)) => Door::Owner(o),
        Ok(Outcome::Forwarded) => Door::Forwarded,
        Err(e) => {
            eprintln!("weft: no single window ({e}); opening a window of its own");
            Door::Closed
        }
    }
}

impl App {
    /// The requests that came to the door since the last frame.
    pub(crate) fn take_door(&mut self, ctx: &egui::Context) {
        let Some(door) = &self.door else { return };
        let reqs: Vec<OpenRequest> = door.try_iter().collect();
        for r in reqs {
            self.open_request(r, ctx);
        }
    }

    /// Open each file as a tab (the caret where it asked) and bring the window
    /// forward, also when it asked for no file.
    pub(crate) fn open_request(&mut self, req: OpenRequest, ctx: &egui::Context) {
        for f in req.files {
            self.open(&f.path);
            let (Some(line), front) = (f.line, self.tabs.front()) else { continue };
            // Only the tab just opened or brought forward takes the caret.
            if front == Some(&tab_id(&f.path)) {
                Goto { line: line as usize, col: f.col.unwrap_or(1) as usize }.apply(self);
            }
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    /// 13.1: The request is the file made absolute against where it was run,
    /// with the position; no file is an empty list (come forward only).
    #[test]
    fn the_request_names_the_file_and_where_to_look() {
        let cwd = std::env::temp_dir().join("weft-door-cwd");
        let r = request(Some(Path::new("a.md")), Some(Goto { line: 3, col: 5 }), &[], &cwd);
        assert_eq!(r.files, vec![FileArg { path: cwd.join("a.md"), line: Some(3), col: Some(5) }]);
        assert_eq!(r.cwd.as_deref(), Some(cwd.as_path()));
        let abs = std::env::temp_dir().join("b.md");
        let r = request(Some(&abs), None, &[], &cwd);
        assert_eq!(r.files, vec![FileArg { path: abs, line: None, col: None }]);
        assert!(request(None, None, &[], &cwd).files.is_empty());
    }

    /// 13.6: `weft a.md b.md c.md`: the others go first in the request, so the
    /// first file asked for ends in front.
    #[test]
    fn more_files_open_behind_the_first() {
        let dir = std::env::temp_dir().join(format!("weft-door-more-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (a, b, c) = (dir.join("a.md"), dir.join("b.md"), dir.join("c.md"));
        for f in [&a, &b, &c] {
            std::fs::write(
                f, "x
",
            )
            .unwrap();
        }
        let mut app = App::for_tests();
        let ctx = egui::Context::default();
        app.open_request(request(Some(&a), None, &[b.clone(), c.clone()], &dir), &ctx);
        assert_eq!(app.tabs.len(), 3);
        assert_eq!(app.tabs.front(), Some(&tab_id(&a)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 13.2: A file that arrives opens as a tab beside the front one, with the
    /// caret where it was asked; the same file again makes no second tab.
    #[test]
    fn a_file_from_outside_opens_as_a_tab() {
        let dir = std::env::temp_dir().join(format!("weft-door-open-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (a, b): (PathBuf, PathBuf) = (dir.join("a.md"), dir.join("b.md"));
        std::fs::write(&a, "one\ntwo\nthree\n").unwrap();
        std::fs::write(&b, "b\n").unwrap();
        let mut app = App::for_tests();
        let ctx = egui::Context::default();
        app.open_request(request(Some(&a), None, &[], &dir), &ctx);
        app.open_request(request(Some(&b), None, &[], &dir), &ctx);
        assert_eq!(app.tabs.len(), 2);
        app.open_request(request(Some(&a), Some(Goto { line: 3, col: 2 }), &[], &dir), &ctx);
        assert_eq!(app.tabs.len(), 2, "no second tab for a.md");
        assert_eq!(app.tabs.front(), Some(&tab_id(&a)));
        let ed = app.doc.editor.as_ref().unwrap();
        assert_eq!(ed.text()[..ed.caret()].matches('\n').count(), 2, "the caret is on line 3");
        // No file: nothing opens, nothing closes.
        app.open_request(request(None, None, &[], &dir), &ctx);
        assert_eq!(app.tabs.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 13.3: A request through a real door reaches the owner's channel; a
    /// second process's `acquire` forwards instead of owning.
    #[test]
    fn a_second_launch_hands_its_file_to_the_first() {
        let dir = std::env::temp_dir().join(format!("weft-door-ipc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let name = "WEFT_DOOR_TEST_13_3";
        let at =
            if cfg!(windows) { format!(r"\\.\pipe\weft-door-test-{}", std::process::id()) } else { dir.join("sock").display().to_string() };
        std::env::set_var(name, &at);
        let req = request(Some(Path::new("a.md")), None, &[], &dir);
        let first = single::acquire("weft", name, PROTO, &req, || {}).unwrap();
        let Outcome::Owner(owner) = first else { panic!("the first owns the door") };
        let again = request(Some(Path::new("b.md")), Some(Goto { line: 2, col: 1 }), &[], &dir);
        assert!(matches!(single::acquire("weft", name, PROTO, &again, || {}).unwrap(), Outcome::Forwarded));
        let got = owner.recv_timeout(std::time::Duration::from_secs(5)).expect("the owner is handed the request");
        assert_eq!(got, again);
        std::env::remove_var(name);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
