use std::path::PathBuf;

use ito_tabs::Op;

use super::tab_id;
use crate::{keymap, App, Quit};

/// A folder with a file per name (each holds its own name), and their tab ids.
fn files(tag: &str, names: &[&str]) -> (PathBuf, Vec<PathBuf>) {
    let dir = std::env::temp_dir().join(format!("weft-tabs-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let paths = names
        .iter()
        .map(|n| {
            let p = dir.join(n);
            std::fs::write(&p, n).unwrap();
            tab_id(&p)
        })
        .collect();
    (dir, paths)
}

fn text(app: &App) -> String {
    app.doc.editor.as_ref().unwrap().text().to_string()
}

fn type_in_front(app: &mut App, s: &str) {
    app.doc.editor.as_mut().unwrap().insert(s, 0);
}

/// Wait for every save the file thread has been given to land.
fn settle(app: &mut App) {
    let ctx = egui::Context::default();
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while app.saving() > 0 && std::time::Instant::now() < until {
        std::thread::sleep(std::time::Duration::from_millis(10));
        app.take_files(&ctx);
    }
    assert_eq!(app.saving(), 0, "the saves landed");
}

/// A file opens as a tab beside the front one and comes forward; the same
/// file again brings its tab forward and makes no second.
#[test]
fn a_file_opens_as_a_tab_once() {
    let (dir, f) = files("open", &["a.md", "b.md"]);
    let mut app = App::for_tests();
    app.open(&f[0]);
    app.open(&f[1]);
    assert_eq!(app.tabs.items(), &f[..]);
    assert_eq!(text(&app), "b.md");
    app.open(&f[0]);
    assert_eq!(app.tabs.len(), 2, "no second tab for a.md");
    assert_eq!(text(&app), "a.md");
    let r = app.report();
    assert!(r.contains("tabs: a.md, b.md\nfront_tab: 1\n"), "{r}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Each tab keeps its own text and unsaved changes while another is in
/// front; the tab commands switch.
#[test]
fn tabs_keep_their_own_changes_while_another_is_in_front() {
    let (dir, f) = files("switch", &["a.md", "b.md", "c.md"]);
    let ctx = egui::Context::default();
    let mut app = App::for_tests();
    for p in &f {
        app.open(p);
    }
    type_in_front(&mut app, "X");
    app.run_key(&ctx, keymap::Cmd::Run("tab.next"));
    assert_eq!(text(&app), "a.md", "next wraps to the first");
    type_in_front(&mut app, "Y");
    app.run_key(&ctx, keymap::Cmd::Run("tab.prev"));
    assert_eq!(text(&app), "Xc.md");
    app.run_key(&ctx, keymap::Cmd::Run("tab.goto_1"));
    assert_eq!(text(&app), "Ya.md");
    let r = app.report();
    assert!(r.contains("tabs: a.md*, b.md, c.md*\n"), "{r}");
    app.tab_op(Op::Goto(7));
    assert_eq!(text(&app), "Ya.md", "no such tab: nothing changes");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Closing a tab with changes asks first; Esc keeps it, N closes it without
/// writing; a clean tab closes at once.
#[test]
fn closing_a_tab_with_changes_asks_first() {
    let (dir, f) = files("close", &["a.md", "b.md"]);
    let mut app = App::for_tests();
    app.open(&f[0]);
    app.open(&f[1]);
    type_in_front(&mut app, "X");
    app.tab_op(Op::Close);
    assert_eq!(app.closing.as_ref(), Some(&f[1]));
    assert!(!app.editing(), "the question has the keys");
    app.answer_close(0);
    assert!(app.closing.is_none() && app.tabs.len() == 2 && app.editing(), "Esc stays");
    app.tab_op(Op::Close);
    app.answer_close(1);
    assert_eq!(app.tabs.items(), &f[..1]);
    assert_eq!(text(&app), "a.md");
    assert_eq!(std::fs::read_to_string(&f[1]).unwrap(), "b.md", "nothing was written");
    app.tab_op(Op::Close);
    assert_eq!(app.tabs.len(), 0, "a clean tab closes at once");
    assert!(app.doc.editor.is_none(), "the last one leaves the window empty");
    let _ = std::fs::remove_dir_all(&dir);
}

/// "Save and close" on a tab that is not in front writes that tab's text to
/// its own file, then closes it; the front tab is left as it was.
#[test]
fn a_save_goes_to_the_tab_it_was_asked_for() {
    let (dir, f) = files("save", &["a.md", "b.md"]);
    let mut app = App::for_tests();
    app.open(&f[0]);
    type_in_front(&mut app, "X");
    app.open(&f[1]);
    app.tab_op(Op::CloseAt(0));
    assert_eq!(app.closing.as_ref(), Some(&f[0]));
    app.answer_close(2);
    settle(&mut app);
    assert_eq!(std::fs::read_to_string(&f[0]).unwrap(), "Xa.md");
    assert_eq!(std::fs::read_to_string(&f[1]).unwrap(), "b.md");
    assert_eq!(app.tabs.items(), &f[1..], "closed once the save landed");
    assert_eq!(text(&app), "b.md");
    assert!(app.parked.is_empty() && !app.any_dirty());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A tab typed in after "Save and close" was asked stays open.
#[test]
fn a_tab_changed_again_before_its_save_landed_stays() {
    let (dir, f) = files("again", &["a.md"]);
    let mut app = App::for_tests();
    app.open(&f[0]);
    type_in_front(&mut app, "X");
    app.tab_op(Op::Close);
    app.answer_close(2);
    type_in_front(&mut app, "Z");
    settle(&mut app);
    assert_eq!(app.tabs.len(), 1);
    assert!(app.doc.dirty(), "the later change is not saved");
    let _ = std::fs::remove_dir_all(&dir);
}

/// With the tabs full, a new file takes the leftmost tab's place; not when
/// that tab has changes not saved (the file stays unopened, told under the bell).
#[test]
fn full_tabs_make_room_only_from_a_saved_tab() {
    let (dir, f) = files("full", &["a.md", "b.md", "c.md", "d.md"]);
    let mut app = App::for_tests();
    app.tabs = ito_tabs::Tabs::new(2);
    app.open(&f[0]);
    app.open(&f[1]);
    app.open(&f[2]);
    assert_eq!(app.tabs.items(), &f[1..3], "a.md made room");
    assert_eq!(app.parked.len(), 1);
    app.doc_mut(&f[1]).unwrap().editor.as_mut().unwrap().insert("X", 0);
    app.open(&f[3]);
    assert_eq!(app.tabs.items(), &f[1..3], "b.md has changes: nothing opens");
    assert_eq!(app.notices.len(), 1, "{:?}", app.notices);
    let title = &app.notices[0].title;
    assert!(title.contains("d.md") && title.contains("b.md"), "{title}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Quitting asks about a tab behind the front one, and "save and quit"
/// writes every tab with changes.
#[test]
fn quitting_asks_about_every_tab() {
    let (dir, f) = files("quit", &["a.md", "b.md", "c.md"]);
    let ctx = egui::Context::default();
    let mut app = App::for_tests();
    for p in &f {
        app.open(p);
    }
    app.doc_mut(&f[0]).unwrap().editor.as_mut().unwrap().insert("X", 0);
    app.doc_mut(&f[1]).unwrap().editor.as_mut().unwrap().insert("Y", 0);
    app.quit(&ctx);
    assert_eq!(app.quit, Some(Quit::Asking));
    assert_eq!(app.quit_body(), "a.md and 1 more have changes that are not saved.");
    app.answer_quit(&ctx, 2);
    settle(&mut app);
    app.quit_after_save(&ctx);
    assert_eq!(app.quit, Some(Quit::Go));
    assert_eq!(std::fs::read_to_string(&f[0]).unwrap(), "Xa.md");
    assert_eq!(std::fs::read_to_string(&f[1]).unwrap(), "Yb.md");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The tabs of the last run come back, in order, with the front one in front;
/// a file that is gone is left out.
#[test]
fn the_last_tabs_come_back_without_the_files_that_are_gone() {
    let (dir, f) = files("restore", &["a.md", "b.md", "c.md"]);
    let mut kept = ito_tabs::Tabs::<PathBuf>::new(12);
    for p in &f {
        kept.open(p.clone());
    }
    kept.apply(Op::Goto(2));
    std::fs::remove_file(&f[1]).unwrap();
    let state = dir.join("tabs.toml");
    std::fs::write(&state, kept.to_toml().unwrap()).unwrap();
    let mut app = App::for_tests();
    app.restore_tabs(&state);
    assert_eq!(app.tabs.items(), &[f[0].clone(), f[2].clone()]);
    assert_eq!(text(&app), "c.md");
    assert_eq!(app.parked.len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}
