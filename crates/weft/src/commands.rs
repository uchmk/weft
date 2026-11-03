//! Every command weft runs, by name ("object.action"). The keymap, the
//! palette and the key help all take their names and descriptions from
//! [`ALL`], so a command is added here once. [`edit`] runs the ones that
//! work on the document; it has no UI, so the tests, `--keys` and the keys
//! reach the same code.

use crate::engine::{Editor, Motion};
use ito_mdedit::{Dir, Step};

/// The keymap layer a command is bound in. `Edit` keys work only while the
/// document has the keys (not in a dialog); `App` keys work everywhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    App,
    Edit,
}

impl Layer {
    pub const fn name(self) -> &'static str {
        match self {
            Layer::App => "app",
            Layer::Edit => "edit",
        }
    }
}

/// One command.
#[derive(Debug)]
pub struct Def {
    pub name: &'static str,
    pub layer: Layer,
    /// Listed in the command palette (moving the caret is not).
    pub palette: bool,
    /// What it does, in English; `[keydesc]` in `lang/*.toml` translates it.
    pub desc: &'static str,
}

const fn app(name: &'static str, desc: &'static str) -> Def {
    Def { name, layer: Layer::App, palette: true, desc }
}

const fn doc(name: &'static str, desc: &'static str) -> Def {
    Def { name, layer: Layer::Edit, palette: true, desc }
}

/// Bound in the edit layer, not in the palette: the caret's moves and the
/// plain keys of typing.
const fn key(name: &'static str, desc: &'static str) -> Def {
    Def { name, layer: Layer::Edit, palette: false, desc }
}

pub const ALL: &[Def] = &[
    app("settings", "Open the settings"),
    app("help", "Show the key help"),
    app("quit", "Quit"),
    app("file.save", "Save the file"),
    app("file.open_in_editor", "Open in the external editor at the caret"),
    app("edit.undo", "Undo"),
    app("edit.redo", "Redo"),
    key("cursor.left", "Move left"),
    key("cursor.right", "Move right"),
    key("cursor.up", "Move up a line"),
    key("cursor.down", "Move down a line"),
    key("cursor.line_start", "To the start of the line"),
    key("cursor.line_end", "To the end of the line"),
    key("cursor.doc_start", "To the start of the document"),
    key("cursor.doc_end", "To the end of the document"),
    key("cursor.page_up", "Up a page"),
    key("cursor.page_down", "Down a page"),
    key("select.left", "Select left"),
    key("select.right", "Select right"),
    key("select.up", "Select up a line"),
    key("select.down", "Select down a line"),
    key("select.line_start", "Select to the start of the line"),
    key("select.line_end", "Select to the end of the line"),
    key("select.doc_start", "Select to the start of the document"),
    key("select.doc_end", "Select to the end of the document"),
    key("select.page_up", "Select up a page"),
    key("select.page_down", "Select down a page"),
    key("edit.backspace", "Delete before the caret"),
    key("edit.delete", "Delete after the caret"),
    key("edit.newline", "Break the line"),
    doc("select.all", "Select all"),
    doc("block.move_up", "Move the block up"),
    doc("block.move_down", "Move the block down"),
    doc("section.move_up", "Move the section up"),
    doc("section.move_down", "Move the section down"),
    doc("section.promote", "Promote the heading"),
    doc("section.demote", "Demote the heading"),
    doc("list.indent", "Indent the list item"),
    doc("list.outdent", "Outdent the list item"),
    doc("list.toggle_task", "Toggle the task"),
    doc("list.renumber", "Renumber the list"),
    doc("table.format", "Format the table"),
    doc("table.next_cell", "To the next cell"),
    doc("table.prev_cell", "To the previous cell"),
];

/// The command named `name`.
pub fn find(name: &str) -> Option<&'static Def> {
    ALL.iter().find(|d| d.name == name)
}

/// The commands the palette lists, in table order.
pub fn palette() -> impl Iterator<Item = &'static Def> {
    ALL.iter().filter(|d| d.palette)
}

/// The caret move `name` makes (`cursor.*`), or extends the selection by
/// (`select.*`); `page` is how many lines a page is.
fn motion(name: &str, page: usize) -> Option<(Motion, bool)> {
    let (object, action) = name.split_once('.')?;
    let extend = match object {
        "cursor" => false,
        "select" => true,
        _ => return None,
    };
    let m = match action {
        "left" => Motion::Left,
        "right" => Motion::Right,
        "up" => Motion::Up,
        "down" => Motion::Down,
        "line_start" => Motion::LineStart,
        "line_end" => Motion::LineEnd,
        "doc_start" => Motion::DocStart,
        "doc_end" => Motion::DocEnd,
        "page_up" => Motion::PageUp(page),
        "page_down" => Motion::PageDown(page),
        _ => return None,
    };
    Some((m, extend))
}

/// Run the document command `name` on `ed`. `Some(true)` when it did
/// something (moved, changed), `Some(false)` when it does not apply here (no
/// table at the caret, nothing to undo, a read-only file), `None` when `name`
/// is not a document command (the app runs those). Every edit is one undo
/// step: a command ends the typing step before it. Backspace and Delete are
/// keys like typing: the ones in a row join into one step.
pub fn edit(ed: &mut Editor, name: &str, now_ms: u64, page: usize) -> Option<bool> {
    if let Some((m, extend)) = motion(name, page) {
        let before = ed.selection();
        ed.move_caret(m, extend);
        return Some(ed.selection() != before);
    }
    match name {
        "edit.backspace" => return Some(ed.backspace(now_ms)),
        "edit.delete" => return Some(ed.delete(now_ms)),
        _ => {}
    }
    ed.seal();
    let done = match name {
        "select.all" => {
            ed.select_all();
            true
        }
        "edit.undo" => ed.undo(),
        "edit.redo" => ed.redo(),
        "edit.newline" => ed.newline(now_ms),
        // A list item moves past its sibling; the first or last item of a
        // list, and anything else, moves as a block.
        "block.move_up" => ed.structure(|t, b| ito_mdedit::move_item(t, b, Dir::Up).or_else(|| ito_mdedit::move_block(t, b, Dir::Up))),
        "block.move_down" => {
            ed.structure(|t, b| ito_mdedit::move_item(t, b, Dir::Down).or_else(|| ito_mdedit::move_block(t, b, Dir::Down)))
        }
        "section.move_up" => ed.structure(|t, b| ito_mdedit::move_section(t, b, Dir::Up)),
        "section.move_down" => ed.structure(|t, b| ito_mdedit::move_section(t, b, Dir::Down)),
        "section.promote" => ed.structure(ito_mdedit::promote_section),
        "section.demote" => ed.structure(ito_mdedit::demote_section),
        "list.indent" => ed.structure(ito_mdedit::indent_item),
        "list.outdent" => ed.structure(ito_mdedit::outdent_item),
        "list.toggle_task" => ed.structure(ito_mdedit::toggle_task),
        "list.renumber" => ed.structure(ito_mdedit::renumber),
        "table.format" => ed.structure(ito_mdedit::format_table),
        "table.next_cell" => ed.structure(|t, b| ito_mdedit::next_cell(t, b, Step::Next)),
        "table.prev_cell" => ed.structure(|t, b| ito_mdedit::next_cell(t, b, Step::Prev)),
        _ => return None,
    };
    // Typing after a command starts a new step.
    ed.seal();
    Some(done)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_object_dot_action_and_unique() {
        let mut seen = std::collections::HashSet::new();
        for d in ALL {
            assert!(seen.insert(d.name), "{} twice", d.name);
            let plain = ["settings", "help", "quit"].contains(&d.name);
            assert!(plain || d.name.split_once('.').is_some_and(|(o, a)| !o.is_empty() && !a.is_empty()), "{}", d.name);
            assert!(!d.desc.is_empty() && !d.desc.ends_with('.'), "{}: {:?}", d.name, d.desc);
        }
    }

    #[test]
    fn every_edit_layer_command_runs_on_the_document() {
        for d in ALL.iter().filter(|d| d.layer == Layer::Edit) {
            let mut ed = Editor::new("# a\n\n- b\n");
            assert!(edit(&mut ed, d.name, 0, 10).is_some(), "{} is not run", d.name);
        }
        let mut ed = Editor::new("");
        for name in ["settings", "help", "quit", "file.save", "file.open_in_editor"] {
            assert_eq!(edit(&mut ed, name, 0, 10), None, "{name} is the app's");
        }
        assert_eq!(edit(&mut ed, "edit.undo", 0, 10), Some(false), "nothing to undo");
    }

    #[test]
    fn moves_select_and_structure_commands_are_one_step_each() {
        let mut ed = Editor::new("- a\n- b\n\nc\n");
        assert_eq!(edit(&mut ed, "select.right", 0, 10), Some(true));
        assert_eq!(ed.selected_text().as_deref(), Some("-"));
        assert_eq!(edit(&mut ed, "cursor.doc_start", 0, 10), Some(true));
        assert_eq!(edit(&mut ed, "cursor.doc_start", 0, 10), Some(false), "already there");
        assert_eq!(edit(&mut ed, "list.toggle_task", 0, 10), Some(true));
        assert_eq!(ed.text(), "- [ ] a\n- b\n\nc\n");
        assert_eq!(edit(&mut ed, "block.move_down", 0, 10), Some(true));
        assert_eq!(ed.text(), "- b\n- [ ] a\n\nc\n", "an item moves past its sibling");
        assert_eq!(edit(&mut ed, "block.move_down", 0, 10), Some(true));
        assert_eq!(ed.text(), "c\n\n- b\n- [ ] a\n", "the last item takes the list past the paragraph");
        assert_eq!(edit(&mut ed, "edit.undo", 0, 10), Some(true));
        assert_eq!(ed.text(), "- b\n- [ ] a\n\nc\n");
        assert_eq!(edit(&mut ed, "edit.undo", 0, 10), Some(true));
        assert_eq!(ed.text(), "- [ ] a\n- b\n\nc\n");
        assert_eq!(edit(&mut ed, "edit.undo", 0, 10), Some(true));
        assert_eq!(ed.text(), "- a\n- b\n\nc\n");
        assert_eq!(edit(&mut ed, "edit.redo", 0, 10), Some(true));
        assert_eq!(ed.text(), "- [ ] a\n- b\n\nc\n");
        assert_eq!(edit(&mut ed, "table.format", 0, 10), Some(false), "no table here");
    }

    #[test]
    fn a_list_item_moves_with_its_children_and_keeps_the_numbers() {
        let mut ed = Editor::new("- a\n  - a1\n- b\n");
        assert_eq!(edit(&mut ed, "block.move_down", 0, 10), Some(true));
        assert_eq!(ed.text(), "- b\n- a\n  - a1\n");
        assert_eq!(&ed.text()[ed.caret()..], "- a\n  - a1\n", "the caret goes with the item");
        let mut ed = Editor::new("1. one\n2. two\n");
        ed.set_caret("1. one\n".len(), false);
        assert_eq!(edit(&mut ed, "block.move_up", 0, 10), Some(true));
        assert_eq!(ed.text(), "1. two\n2. one\n");
    }

    #[test]
    fn backspaces_in_a_row_are_one_step_and_a_command_ends_them() {
        let mut ed = Editor::new("abcd");
        ed.set_caret(4, false);
        assert_eq!(edit(&mut ed, "edit.backspace", 0, 10), Some(true));
        assert_eq!(edit(&mut ed, "edit.backspace", 100, 10), Some(true));
        assert_eq!(ed.text(), "ab");
        assert_eq!(edit(&mut ed, "edit.undo", 200, 10), Some(true));
        assert_eq!(ed.text(), "abcd", "both come back as one");
        ed.set_caret(4, false);
        edit(&mut ed, "edit.backspace", 300, 10);
        assert_eq!(edit(&mut ed, "table.format", 400, 10), Some(false), "a command that does nothing still ends the run");
        edit(&mut ed, "edit.backspace", 500, 10);
        assert_eq!(ed.text(), "ab");
        edit(&mut ed, "edit.undo", 600, 10);
        assert_eq!(ed.text(), "abc", "a command between them splits the run");
    }

    #[test]
    fn a_command_ends_the_typing_step() {
        let mut ed = Editor::new("");
        ed.insert("a", 0);
        assert_eq!(edit(&mut ed, "edit.newline", 10, 10), Some(true));
        ed.insert("b", 20);
        assert_eq!(ed.text(), "a\nb");
        edit(&mut ed, "edit.undo", 30, 10);
        assert_eq!(ed.text(), "a\n", "the b is a step of its own");
        edit(&mut ed, "edit.undo", 30, 10);
        assert_eq!(ed.text(), "a", "the break is a step of its own");
    }
}
