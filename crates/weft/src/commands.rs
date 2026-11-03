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
    doc("edit.tab", "Next cell, indent the item or put in spaces"),
    doc("edit.untab", "Previous cell, outdent the item or take spaces away"),
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
        // Tab: in a table the next cell (past the last one, a new row); on a
        // list item the item one level in; anywhere else the tab width in
        // spaces (before each line of a selection that crosses a break).
        "edit.tab" => {
            let width = ed.tab_width();
            ed.structure(|t, b| ito_mdedit::next_cell(t, b, Step::Next).or_else(|| ito_mdedit::indent_item(t, b))) || ed.indent(width)
        }
        // Shift+Tab undoes what Tab does where it is: in a table the cell
        // before (nothing at the first), on a list item the item one level
        // out (nothing at the top level), anywhere else up to the tab width
        // of spaces off the start of the line (of each line selected).
        "edit.untab" => {
            let (text, caret) = (ed.text(), ed.caret());
            if ito_mdedit::next_cell(text, caret, Step::Next).is_some() {
                ed.structure(|t, b| ito_mdedit::next_cell(t, b, Step::Prev))
            } else if ito_mdedit::indent_item(text, caret).is_some() {
                ed.structure(ito_mdedit::outdent_item)
            } else {
                ed.outdent(ed.tab_width())
            }
        }
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

    fn at(text: &str, caret: usize) -> Editor {
        let mut ed = Editor::new(text);
        ed.set_caret(caret, false);
        ed
    }

    /// 8.11: Tab and Shift+Tab move between the cells of a table, indent and
    /// outdent a list item, and put in or take away spaces anywhere else.
    #[test]
    fn tab_and_shift_tab_go_by_table_then_list_item_then_spaces() {
        // A table (lined up, as a move between cells leaves it): the next
        // cell, the one before; nothing before the first.
        let table = "| a   | b   |\n| --- | --- |\n| c   | d   |\n";
        let mut ed = at(table, 2);
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert!(ed.text()[ed.caret()..].starts_with("b   |"), "{:?}", &ed.text()[ed.caret()..]);
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert!(ed.text()[ed.caret()..].starts_with("c   |"), "past the header, over the delimiter row");
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(true));
        assert!(ed.text()[ed.caret()..].starts_with("b   |"));
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(true));
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(false), "at the first cell, no spaces either");
        assert_eq!(ed.text(), table);

        // A list item: one level in, one out; nothing out at the left edge.
        let mut ed = at("- a\n- b\n  - c\n", "- a\n- ".len());
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!(ed.text(), "- a\n  - b\n    - c\n", "the item takes its children");
        assert_eq!(&ed.text()[ed.caret()..], "b\n    - c\n", "the caret stays on its word");
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(true));
        assert_eq!(ed.text(), "- a\n- b\n  - c\n");
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(false), "a top-level item takes no spaces away");
        assert_eq!(ed.text(), "- a\n- b\n  - c\n");

        // Anywhere else: the tab width in spaces at the caret, and back.
        let mut ed = at("ab", 1);
        assert_eq!(ed.tab_width(), 4, "4 until the settings say otherwise");
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!(ed.text(), "a    b");
        assert_eq!(ed.caret(), 5);
        let mut ed = at("ab", 1);
        ed.set_tab_width(2);
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!(ed.text(), "a  b");
        assert_eq!(ed.caret(), 3);
        let mut ed = at("     x", 6);
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(true));
        assert_eq!(ed.text(), " x", "up to the tab width off the start of the line, wherever the caret is");
        assert_eq!(ed.caret(), 2);
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(true));
        assert_eq!(ed.text(), "x", "fewer than the width: what there is");
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(false), "no space, nothing to do");
        let mut ed = at("  x", 1);
        ed.set_tab_width(2);
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(true));
        assert_eq!((ed.text(), ed.caret()), ("x", 0), "a caret among the spaces goes to the start");
    }

    #[test]
    fn tab_over_a_selection_of_lines_indents_each_and_shift_tab_takes_it_back() {
        let mut ed = at("a\nb\n\nc", 0);
        ed.set_tab_width(2);
        edit(&mut ed, "select.all", 0, 10);
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!(ed.text(), "  a\n  b\n\n  c", "an empty line gets no spaces");
        assert_eq!(ed.selected_text().as_deref(), Some("  a\n  b\n\n  c"), "the selection takes in the spaces");
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(true));
        assert_eq!(ed.text(), "a\nb\n\nc");
        assert_eq!(ed.selected_text().as_deref(), Some("a\nb\n\nc"));
        // A selection that ends at the start of a line leaves that line be.
        let mut ed = at("a\nb\nc", 0);
        ed.set_caret("a\nb\n".len(), true);
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!(ed.text(), "    a\n    b\nc");
        assert_eq!(ed.selected_text().as_deref(), Some("    a\n    b\n"));
        // Within one line, the spaces take the selection's place.
        let mut ed = at("abcd", 1);
        ed.set_caret(3, true);
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!((ed.text(), ed.caret()), ("a    d", 5));
    }

    #[test]
    fn tab_and_shift_tab_keep_crlf_breaks() {
        let mut ed = at("x\r\ny\r\n", 0);
        edit(&mut ed, "select.all", 0, 10);
        ed.set_tab_width(2);
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!(ed.text(), "  x\r\n  y\r\n");
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(true));
        assert_eq!(ed.text(), "x\r\ny\r\n");
        let mut ed = at("x\r\n  y\r\n", "x\r\n".len());
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(true));
        assert_eq!(ed.text(), "x\r\ny\r\n");
        let mut ed = at("- a\r\n- b\r\n", "- a\r\n- ".len());
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!(ed.text(), "- a\r\n  - b\r\n");
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(true));
        assert_eq!(ed.text(), "- a\r\n- b\r\n");
    }

    #[test]
    fn each_tab_and_shift_tab_is_one_undo_step() {
        let mut ed = at("a\nb", 0);
        edit(&mut ed, "select.all", 0, 10);
        edit(&mut ed, "edit.tab", 0, 10);
        edit(&mut ed, "edit.tab", 10, 10);
        assert_eq!(ed.text(), "        a\n        b");
        assert_eq!(edit(&mut ed, "edit.undo", 20, 10), Some(true));
        assert_eq!(ed.text(), "    a\n    b", "every line of a press comes back at once, one press at a time");
        assert_eq!(ed.selected_text().as_deref(), Some("    a\n    b"));
        edit(&mut ed, "edit.untab", 30, 10);
        assert_eq!(ed.text(), "a\nb");
        assert_eq!(edit(&mut ed, "edit.undo", 40, 10), Some(true));
        assert_eq!(ed.text(), "    a\n    b");
        // Typing after a Tab is a step of its own.
        let mut ed = at("", 0);
        ed.insert("a", 0);
        edit(&mut ed, "edit.tab", 10, 10);
        ed.insert("b", 20);
        assert_eq!(ed.text(), "a    b");
        edit(&mut ed, "edit.undo", 30, 10);
        assert_eq!(ed.text(), "a    ");
        edit(&mut ed, "edit.undo", 30, 10);
        assert_eq!(ed.text(), "a");
        // A list item and a cell, one step each.
        let mut ed = at("- a\n- b\n", "- a\n".len());
        edit(&mut ed, "edit.tab", 0, 10);
        edit(&mut ed, "edit.tab", 0, 10);
        assert_eq!(ed.text(), "- a\n    - b\n");
        edit(&mut ed, "edit.undo", 0, 10);
        assert_eq!(ed.text(), "- a\n  - b\n");
    }

    #[test]
    fn tab_and_shift_tab_change_nothing_in_a_read_only_file() {
        let dir = std::env::temp_dir().join(format!("weft-tab-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for (name, bytes) in
            [("plain.md", &b"\x82\xa0  abc"[..]), ("list.md", b"- a\n- \x82\xa0"), ("table.md", b"| \x82\xa0 | b |\n| - | - |\n")]
        {
            let path = dir.join(name);
            std::fs::write(&path, bytes).unwrap();
            let mut ed = Editor::open(&path).unwrap();
            assert!(ed.locked().is_some(), "{name}");
            let text = ed.text().to_owned();
            for caret in [0, text.len()] {
                ed.set_caret(caret, false);
                assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(false), "{name}");
                assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(false), "{name}");
            }
            assert_eq!(ed.text(), text, "{name}");
            assert_eq!(edit(&mut ed, "edit.undo", 0, 10), Some(false), "{name}: nothing was done");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A table in a list item stays in the item: every row keeps its indent,
    /// a new row past the last cell too (uchmk/ito#170).
    #[test]
    fn tab_in_a_table_inside_a_list_item_keeps_the_rows_indent() {
        let t = "- item\n  | a | b |\n  | --- | --- |\n  | c | d |\n";
        let lined_up = "- item\n  | a   | b   |\n  | --- | --- |\n  | c   | d   |\n";
        let mut ed = at(t, t.find('a').unwrap());
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!(ed.text(), lined_up);
        assert!(ed.text()[ed.caret()..].starts_with("b   |"), "{:?}", &ed.text()[ed.caret()..]);
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert!(ed.text()[ed.caret()..].starts_with("c   |"), "over the delimiter row");
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(true));
        assert!(ed.text()[ed.caret()..].starts_with("b   |"));
        assert_eq!(ed.text(), lined_up);
        ed.set_caret(lined_up.find('d').unwrap(), false);
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!(ed.text(), "- item\n  | a   | b   |\n  | --- | --- |\n  | c   | d   |\n  |     |     |\n", "the new row too");
        assert!(ed.text()[..ed.caret()].ends_with("\n  | "), "the caret in the new row's first cell");
    }

    /// A CRLF table is written back with CRLF between its rows, a new row
    /// too, and in a list item as well (uchmk/ito#170).
    #[test]
    fn tab_in_a_crlf_table_keeps_crlf_inside_the_table() {
        let t = "| a | b |\r\n|---|---|\r\n| 1 | 2 |\r\n";
        let mut ed = at(t, 2);
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!(ed.text(), "| a   | b   |\r\n| --- | --- |\r\n| 1   | 2   |\r\n");
        assert!(ed.text()[ed.caret()..].starts_with("b   |"));
        ed.set_caret(ed.text().find('2').unwrap(), false);
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!(ed.text(), "| a   | b   |\r\n| --- | --- |\r\n| 1   | 2   |\r\n|     |     |\r\n");
        let t = "- x\r\n  | a | b |\r\n  |---|---|\r\n";
        let mut ed = at(t, t.find('a').unwrap());
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!(ed.text(), "- x\r\n  | a   | b   |\r\n  | --- | --- |\r\n");
    }

    /// In a code fence a `- x` line is code, not a list item: Tab puts the
    /// spaces in at the caret and Shift+Tab takes the tab width of spaces off
    /// the line, as in a paragraph (uchmk/ito#171).
    #[test]
    fn tab_on_a_list_line_in_a_code_fence_puts_spaces_at_the_caret() {
        let t = "```\n- x\n```\n";
        let mut ed = at(t, "```\n- ".len());
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!(ed.text(), "```\n-     x\n```\n");
        assert_eq!(ed.caret(), "```\n-     ".len());
        let t = "```\n    - x\n```\n";
        let mut ed = at(t, t.find('x').unwrap());
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(true));
        assert_eq!(ed.text(), "```\n- x\n```\n", "the tab width of spaces, not a list level");
    }

    /// Tab makes a numbered item the first of a new sublist (from 1) and the
    /// list it left goes on numbered; Shift+Tab puts it back in the numbers.
    /// Each press is one undo step, the numbers with it (uchmk/ito#171).
    #[test]
    fn tab_and_shift_tab_on_a_numbered_item_keep_both_lists_numbered_from_their_start() {
        let t = "1. a\n2. b\n";
        let mut ed = at(t, t.find('b').unwrap());
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!(ed.text(), "1. a\n   1. b\n");
        assert_eq!(&ed.text()[ed.caret()..], "b\n", "the caret stays on its word");
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(true));
        assert_eq!(ed.text(), "1. a\n2. b\n");
        assert_eq!(&ed.text()[ed.caret()..], "b\n");

        let t = "1. a\n2. b\n3. c\n";
        let mut ed = at(t, t.find('b').unwrap());
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!(ed.text(), "1. a\n   1. b\n2. c\n");
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(true));
        assert_eq!(ed.text(), "1. a\n2. b\n3. c\n");
        assert_eq!(edit(&mut ed, "edit.undo", 0, 10), Some(true));
        assert_eq!(ed.text(), "1. a\n   1. b\n2. c\n");
        assert_eq!(edit(&mut ed, "edit.undo", 0, 10), Some(true));
        assert_eq!(ed.text(), t);
    }

    /// An item indented with a tab is a list item: Tab takes it one level in,
    /// Shift+Tab out to the left edge (uchmk/ito#171).
    #[test]
    fn tab_on_a_tab_indented_item_indents_it_as_an_item() {
        let t = "- top\n\t- tabbed\n";
        let mut ed = at(t, t.find("tabbed").unwrap());
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!(ed.text(), "- top\n      - tabbed\n", "the tab is read as 4 and written as spaces, 2 more");
        assert_eq!(&ed.text()[ed.caret()..], "tabbed\n");
        let mut ed = at(t, t.find("tabbed").unwrap());
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(true));
        assert_eq!(ed.text(), "- top\n- tabbed\n");
        assert_eq!(&ed.text()[ed.caret()..], "tabbed\n");
    }
}
