//! Every command weft runs, by name ("object.action"). The keymap, the
//! palette and the key help all take their names and descriptions from
//! [`ALL`], so a command is added here once. [`edit`] runs the ones that
//! work on the document; it has no UI, so the tests, `--keys` and the keys
//! reach the same code.

use std::ops::Range;

use crate::engine::{Editor, Motion};
use ito_mdedit::{Dir, Inline, Line, SelChange, Step};

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
    doc("edit.tab", "Next cell, indent the item or the lines, or put in a tab"),
    doc("edit.untab", "Previous cell, outdent the item or the lines, or take a tab away"),
    doc("inline.bold", "Toggle bold"),
    doc("inline.italic", "Toggle italic"),
    doc("inline.code", "Toggle inline code"),
    doc("inline.strike", "Toggle strikethrough"),
    doc("inline.link", "Toggle a link"),
    doc("heading.set_1", "Toggle heading 1"),
    doc("heading.set_2", "Toggle heading 2"),
    doc("heading.set_3", "Toggle heading 3"),
    doc("heading.set_4", "Toggle heading 4"),
    doc("heading.set_5", "Toggle heading 5"),
    doc("heading.set_6", "Toggle heading 6"),
    doc("heading.clear", "Turn the headings into paragraphs"),
    doc("block.quote", "Toggle the quote"),
    doc("block.fence", "Toggle the code block"),
    doc("list.bullet", "Toggle the bulleted list"),
    doc("list.number", "Toggle the numbered list"),
    doc("list.task", "Toggle the task list"),
    doc("list.check_tasks", "Check the selected tasks"),
    doc("list.uncheck_tasks", "Uncheck the selected tasks"),
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

/// A format command of ito-mdedit: an inline markup or a line type.
#[derive(Clone, Copy)]
enum Format {
    Inline(Inline),
    Line(Line),
}

impl Format {
    fn run(self, text: &str, sel: Range<usize>) -> Option<SelChange> {
        match self {
            Format::Inline(k) => ito_mdedit::inline(text, sel, k),
            Format::Line(k) => ito_mdedit::line_type(text, sel, k),
        }
    }
}

/// The format command `name` runs (`inline.*`, `heading.set_N`,
/// `heading.clear`, `block.quote`, `block.fence`, and the list types).
fn format(name: &str) -> Option<Format> {
    let f = match name {
        "inline.bold" => Format::Inline(Inline::Bold),
        "inline.italic" => Format::Inline(Inline::Italic),
        "inline.code" => Format::Inline(Inline::Code),
        "inline.strike" => Format::Inline(Inline::Strike),
        "inline.link" => Format::Inline(Inline::Link),
        // Heading N on lines that are all heading N makes them paragraphs.
        "heading.set_1" => Format::Line(Line::Heading(1)),
        "heading.set_2" => Format::Line(Line::Heading(2)),
        "heading.set_3" => Format::Line(Line::Heading(3)),
        "heading.set_4" => Format::Line(Line::Heading(4)),
        "heading.set_5" => Format::Line(Line::Heading(5)),
        "heading.set_6" => Format::Line(Line::Heading(6)),
        "heading.clear" => Format::Line(Line::Heading(0)),
        "block.quote" => Format::Line(Line::Quote),
        "block.fence" => Format::Line(Line::Fence),
        "list.bullet" => Format::Line(Line::Bullet),
        "list.number" => Format::Line(Line::Number),
        "list.task" => Format::Line(Line::Task),
        _ => return None,
    };
    Some(f)
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
        // A caret flips the box of its line (or gives a plain item one); a
        // selection checks every task it touches, or unchecks them when all
        // are checked, and falls back to the caret's line without a task.
        "list.toggle_task" if ed.selection().is_empty() => ed.structure(ito_mdedit::toggle_task),
        "list.toggle_task" => ed.format(ito_mdedit::toggle_tasks) || ed.structure(ito_mdedit::toggle_task),
        "list.renumber" => ed.structure(ito_mdedit::renumber),
        "table.format" => ed.structure(ito_mdedit::format_table),
        "table.next_cell" => ed.structure(|t, b| ito_mdedit::next_cell(t, b, Step::Next)),
        "table.prev_cell" => ed.structure(|t, b| ito_mdedit::next_cell(t, b, Step::Prev)),
        // Tab over a selection that crosses a line break: each line one
        // level in (a list item with its nested lines, a table's rows only
        // inside an item, blank lines not). Otherwise, in a table the next
        // cell (past the last one, a new row); on a list item the item one
        // level in; anywhere else a tab or the tab width in spaces.
        "edit.tab" => {
            if ed.spans_lines() {
                ed.shift_lines(true)
            } else {
                ed.structure(|t, b| ito_mdedit::next_cell(t, b, Step::Next).or_else(|| ito_mdedit::indent_item(t, b))) || ed.indent()
            }
        }
        // Shift+Tab undoes what Tab does where it is: over lines, each line
        // one level out; in a table the cell before (nothing at the first),
        // on a list item the item one level out (nothing at the top level),
        // anywhere else a tab or up to the tab width of spaces off the start
        // of the line.
        "edit.untab" => {
            let (text, caret) = (ed.text(), ed.caret());
            if ed.spans_lines() {
                ed.shift_lines(false)
            } else if ito_mdedit::next_cell(text, caret, Step::Next).is_some() {
                ed.structure(|t, b| ito_mdedit::next_cell(t, b, Step::Prev))
            } else if ito_mdedit::indent_item(text, caret).is_some() {
                ed.structure(ito_mdedit::outdent_item)
            } else {
                ed.outdent()
            }
        }
        "list.check_tasks" => ed.format(ito_mdedit::check_tasks),
        "list.uncheck_tasks" => ed.format(ito_mdedit::uncheck_tasks),
        _ => {
            let f = format(name)?;
            ed.format(|t, s| f.run(t, s))
        }
    };
    // Typing after a command starts a new step.
    ed.seal();
    Some(done)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ito_textbuf::Selection;

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

    fn select(text: &str, from: usize, to: usize) -> Editor {
        let mut ed = at(text, from);
        ed.set_caret(to, true);
        ed
    }

    /// 8.16: Tab over several lines takes each one level in: a list item as
    /// an item, any other line by the tab width, an empty line not at all;
    /// Shift+Tab takes them back out. The selection stays on its lines.
    #[test]
    fn tab_over_items_and_paragraph_lines_takes_each_one_level_in_and_back() {
        let t = "para\n- a\n- b\n";
        let mut ed = at(t, 0);
        edit(&mut ed, "select.all", 0, 10);
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!(ed.text(), "    para\n  - a\n  - b\n", "the items by a list level, the paragraph by the tab width");
        assert_eq!(ed.selected_text().as_deref(), Some(ed.text()));
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(true));
        assert_eq!(ed.text(), t);
        assert_eq!(ed.selected_text().as_deref(), Some(t));

        // Ends inside lines: they stay where they were among the words.
        let t = "x\npara\n- a\n- b\ny\n";
        let mut ed = select(t, t.find("ra").unwrap(), t.find("b\n").unwrap());
        assert_eq!(ed.selected_text().as_deref(), Some("ra\n- a\n- "));
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!(ed.text(), "x\n    para\n  - a\n  - b\ny\n", "only the lines the selection is on");
        assert_eq!(ed.selected_text().as_deref(), Some("ra\n  - a\n  - "));
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(true));
        assert_eq!(ed.text(), t);
        assert_eq!(ed.selected_text().as_deref(), Some("ra\n- a\n- "));

        // A selection that ends at the start of a line leaves that line be.
        let t = "- a\n- b\n- c\n";
        let mut ed = select(t, 0, "- a\n- b\n".len());
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!(ed.text(), "  - a\n  - b\n- c\n");
        assert_eq!(ed.selected_text().as_deref(), Some("  - a\n  - b\n"));

        // Pressed again, it goes on from where it is.
        let t = "para\n- a\n- b";
        let mut ed = at(t, 0);
        edit(&mut ed, "select.all", 0, 10);
        edit(&mut ed, "edit.tab", 0, 10);
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!(ed.text(), "        para\n    - a\n    - b");
        assert_eq!(ed.selected_text().as_deref(), Some(ed.text()));
    }

    /// 8.16: With a parent item and its child both selected, the parent goes
    /// and takes the child: the child is not moved a second time. A top-level
    /// item does not go out, and neither do the items under it.
    #[test]
    fn tab_over_a_parent_and_its_child_moves_the_child_once() {
        let t = "- a\n  - b\n    more\n- c\n";
        let mut ed = at(t, 0);
        edit(&mut ed, "select.all", 0, 10);
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!(ed.text(), "  - a\n    - b\n      more\n  - c\n");
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(true));
        assert_eq!(ed.text(), t);
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(false), "a and c are at the left edge: b stays under a");
        assert_eq!(ed.text(), t);
        // The child alone, with the line after it.
        let mut ed = select(t, t.find("- b").unwrap(), t.len());
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(true));
        assert_eq!(ed.text(), "- a\n- b\n  more\n- c\n", "b goes out with its line; c is at the edge already");
        // Numbered: the numbers follow.
        let t = "1. a\n2. b\n3. c\n";
        let mut ed = select(t, t.find('b').unwrap(), t.len());
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!(ed.text(), "1. a\n   1. b\n   2. c\n");
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(true));
        assert_eq!(ed.text(), t);
    }

    /// 8.16: In a code fence every line is code: the fence lines and a `- x`
    /// or table line in it all get the tab width, as text.
    #[test]
    fn tab_over_a_code_fence_indents_every_line_as_text() {
        let t = "```\n- x\n| a |\n| - |\n```\n";
        let mut ed = at(t, 0);
        edit(&mut ed, "select.all", 0, 10);
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!(ed.text(), "    ```\n    - x\n    | a |\n    | - |\n    ```\n");
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(true));
        assert_eq!(ed.text(), t);
    }

    /// 8.16: A table's rows do not go in or out on their own: only with a
    /// selected list item they are in. The lines round them still do.
    #[test]
    fn tab_over_lines_leaves_a_tables_own_rows_and_moves_one_in_an_item() {
        let table = "| a | b |\n| --- | --- |\n| c | d |\n";
        let mut ed = at(table, 0);
        edit(&mut ed, "select.all", 0, 10);
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(false), "nothing but table rows: nothing to do");
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(false));
        assert_eq!(ed.text(), table);
        let t = format!("para\n\n{table}\nend\n");
        let mut ed = at(&t, 0);
        edit(&mut ed, "select.all", 0, 10);
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!(ed.text(), format!("    para\n\n{table}\n    end\n"));
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(true));
        assert_eq!(ed.text(), t);
        let t = "- item\n  | a | b |\n  | --- | --- |\n  | c | d |\n";
        let mut ed = at(t, 0);
        edit(&mut ed, "select.all", 0, 10);
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!(ed.text(), "  - item\n    | a | b |\n    | --- | --- |\n    | c | d |\n", "the table goes with its item");
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(true));
        assert_eq!(ed.text(), t);
    }

    /// 8.16: CRLF lines keep their CRLF, items and other lines alike.
    #[test]
    fn tab_over_crlf_lines_keeps_crlf() {
        let t = "para\r\n- a\r\n  - b\r\n";
        let mut ed = at(t, 0);
        edit(&mut ed, "select.all", 0, 10);
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!(ed.text(), "    para\r\n  - a\r\n    - b\r\n");
        assert_eq!(ed.selected_text().as_deref(), Some(ed.text()));
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(true));
        assert_eq!(ed.text(), t);
    }

    /// 8.16: One press over several lines is one undo step: the text and the
    /// selection come back as they were, one press at a time.
    #[test]
    fn tab_over_lines_is_one_undo_step_with_the_selection() {
        let t = "x\npara\n- a\n- b\ny\n";
        let (from, to) = (t.find("ra").unwrap(), t.find("b\n").unwrap());
        let mut ed = select(t, from, to);
        edit(&mut ed, "edit.tab", 0, 10);
        edit(&mut ed, "edit.tab", 10, 10);
        assert_eq!(ed.text(), "x\n        para\n    - a\n    - b\ny\n");
        assert_eq!(edit(&mut ed, "edit.undo", 20, 10), Some(true));
        assert_eq!(ed.text(), "x\n    para\n  - a\n  - b\ny\n");
        assert_eq!(ed.selected_text().as_deref(), Some("ra\n  - a\n  - "));
        assert_eq!(edit(&mut ed, "edit.undo", 30, 10), Some(true));
        assert_eq!(ed.text(), t);
        assert_eq!(ed.selected_text().as_deref(), Some("ra\n- a\n- "));
        assert_eq!(edit(&mut ed, "edit.redo", 40, 10), Some(true));
        assert_eq!(ed.text(), "x\n    para\n  - a\n  - b\ny\n");
        assert_eq!(ed.selected_text().as_deref(), Some("ra\n  - a\n  - "));
        edit(&mut ed, "edit.untab", 50, 10);
        assert_eq!(ed.text(), t);
        assert_eq!(edit(&mut ed, "edit.undo", 60, 10), Some(true));
        assert_eq!(ed.text(), "x\n    para\n  - a\n  - b\ny\n", "Shift+Tab too");
    }

    /// 8.17: With the tab character on (the settings), Tab puts in a tab and
    /// Shift+Tab takes one away (or up to the tab width of spaces); over
    /// several lines each line gets one at its start. List items and table
    /// cells are as before.
    #[test]
    fn with_the_tab_character_tab_puts_in_a_tab_and_shift_tab_takes_one_away() {
        let mut ed = at("ab", 1);
        ed.set_tab_char(true);
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!((ed.text(), ed.caret()), ("a\tb", 2));
        let mut ed = at("abcd", 1);
        ed.set_tab_char(true);
        ed.set_caret(3, true);
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!((ed.text(), ed.caret()), ("a\td", 2), "the tab takes the selection's place");
        let mut ed = at("\t\tx", 3);
        ed.set_tab_char(true);
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(true));
        assert_eq!((ed.text(), ed.caret()), ("\tx", 2), "one tab, not both");
        let mut ed = at("      x", 7);
        ed.set_tab_char(true);
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(true));
        assert_eq!(ed.text(), "  x", "spaces: up to the tab width");

        let t = "a\nb\n\n- c\n";
        let mut ed = at(t, 0);
        ed.set_tab_char(true);
        edit(&mut ed, "select.all", 0, 10);
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!(ed.text(), "\ta\n\tb\n\n  - c\n", "the item as an item, in spaces");
        assert_eq!(ed.selected_text().as_deref(), Some(ed.text()));
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(true));
        assert_eq!(ed.text(), t);
        assert_eq!(edit(&mut ed, "edit.undo", 0, 10), Some(true));
        assert_eq!(edit(&mut ed, "edit.undo", 0, 10), Some(true));
        assert_eq!(ed.text(), t);

        let mut ed = at("- a\n- b\n", "- a\n- ".len());
        ed.set_tab_char(true);
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!(ed.text(), "- a\n  - b\n");
        let table = "| a   | b   |\n| --- | --- |\n";
        let mut ed = at(table, 2);
        ed.set_tab_char(true);
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!((ed.text(), &ed.text()[ed.caret()..ed.caret() + 1]), (table, "b"));
    }

    /// An editor over `text` with `anchor..head` selected (the caret at `head`).
    fn selected(text: &str, anchor: usize, head: usize) -> Editor {
        let mut ed = Editor::new(text);
        ed.set_caret(anchor, false);
        ed.set_caret(head, true);
        ed
    }

    /// 10.1: Bold over three list items marks each item's text, not its
    /// marker, and keeps the same text selected; a second run takes the marks
    /// off again.
    #[test]
    fn bold_over_list_items_marks_each_and_a_second_run_takes_it_off() {
        let text = "- a\n- b\n- c\n";
        let mut ed = selected(text, 0, "- a\n- b\n- c".len());
        assert_eq!(edit(&mut ed, "inline.bold", 0, 10), Some(true));
        assert_eq!(ed.text(), "- **a**\n- **b**\n- **c**\n");
        assert_eq!(ed.selected_text().as_deref(), Some("a**\n- **b**\n- **c"));
        let sel = ed.selection();
        assert!(sel.anchor < sel.head, "the caret stays at the end it was at: {sel:?}");
        assert_eq!(edit(&mut ed, "inline.bold", 0, 10), Some(true));
        assert_eq!(ed.text(), text);
        assert_eq!(ed.selected_text().as_deref(), Some("a\n- b\n- c"));
    }

    /// 10.2: A format command is one undo step: one undo gives back the text
    /// and the selection it ran on (in its direction), and redo the new text
    /// with its selection.
    #[test]
    fn one_undo_takes_back_a_format_command_with_its_selection() {
        let text = "one\ntwo\n";
        // Selected backwards: the caret at the start.
        let mut ed = selected(text, "one\ntwo".len(), 0);
        let before = ed.selection();
        assert_eq!(edit(&mut ed, "list.number", 0, 10), Some(true));
        assert_eq!(ed.text(), "1. one\n2. two\n");
        let after = ed.selection();
        assert!(after.head < after.anchor, "still selected backwards: {after:?}");
        assert_eq!(edit(&mut ed, "edit.undo", 0, 10), Some(true));
        assert_eq!(ed.text(), text);
        assert_eq!(ed.selection(), before);
        assert_eq!(edit(&mut ed, "edit.undo", 0, 10), Some(false), "it was one step");
        assert_eq!(edit(&mut ed, "edit.redo", 0, 10), Some(true));
        assert_eq!(ed.text(), "1. one\n2. two\n");
        assert_eq!(ed.selection(), after);
    }

    /// 10.3: Heading 3 on a paragraph makes it one; heading 1 changes the
    /// level; heading 1 again makes it a paragraph, the caret in its word all
    /// along. "Turn the headings into paragraphs" takes any level off and
    /// does nothing to a paragraph.
    #[test]
    fn a_heading_level_changes_and_comes_back() {
        let mut ed = Editor::new("Title\n\nbody\n");
        ed.set_caret(2, false);
        assert_eq!(edit(&mut ed, "heading.set_3", 0, 10), Some(true));
        assert_eq!(ed.text(), "### Title\n\nbody\n");
        assert_eq!(ed.caret(), "### Ti".len());
        assert_eq!(edit(&mut ed, "heading.set_1", 0, 10), Some(true));
        assert_eq!(ed.text(), "# Title\n\nbody\n");
        assert_eq!(ed.caret(), "# Ti".len());
        assert_eq!(edit(&mut ed, "heading.set_1", 0, 10), Some(true));
        assert_eq!(ed.text(), "Title\n\nbody\n", "the same level again is a paragraph");
        assert_eq!(ed.caret(), 2);
        assert_eq!(edit(&mut ed, "heading.clear", 0, 10), Some(false), "a paragraph already");
        assert_eq!(edit(&mut ed, "heading.set_6", 0, 10), Some(true));
        assert_eq!(ed.text(), "###### Title\n\nbody\n");
        assert_eq!(edit(&mut ed, "heading.clear", 0, 10), Some(true));
        assert_eq!(ed.text(), "Title\n\nbody\n");
    }

    /// 10.4: Quote over lines with a blank one between marks every line (the
    /// blank one with a bare `>`, so they stay one quote); again takes it off.
    #[test]
    fn a_quote_over_several_lines_and_off_again() {
        let text = "a\n\nb\n";
        let mut ed = selected(text, 0, "a\n\nb".len());
        assert_eq!(edit(&mut ed, "block.quote", 0, 10), Some(true));
        assert_eq!(ed.text(), "> a\n>\n> b\n");
        assert_eq!(ed.selected_text().as_deref(), Some("> a\n>\n> b"));
        assert_eq!(edit(&mut ed, "block.quote", 0, 10), Some(true));
        assert_eq!(ed.text(), text);
        assert_eq!(ed.selected_text().as_deref(), Some("a\n\nb"));
    }

    /// 10.5: Code block wraps the selected lines in a fence; run with the
    /// selection (or the caret) inside a fence, it takes the fence off.
    #[test]
    fn a_fence_wraps_the_lines_and_comes_off_again() {
        let text = "a\nb\n";
        let mut ed = selected(text, 0, "a\nb".len());
        assert_eq!(edit(&mut ed, "block.fence", 0, 10), Some(true));
        assert_eq!(ed.text(), "```\na\nb\n```\n");
        assert_eq!(ed.selected_text().as_deref(), Some("a\nb"));
        assert_eq!(edit(&mut ed, "block.fence", 0, 10), Some(true));
        assert_eq!(ed.text(), text);
        assert_eq!(ed.selected_text().as_deref(), Some("a\nb"));
        let mut ed = Editor::new("```rust\nfn main() {}\n```\n");
        ed.set_caret("```rust\nfn".len(), false);
        assert_eq!(edit(&mut ed, "block.fence", 0, 10), Some(true));
        assert_eq!(ed.text(), "fn main() {}\n");
        assert_eq!(ed.caret(), "fn".len());
    }

    /// 10.6: Check and uncheck the selected tasks touch only the tasks (a
    /// plain item and a paragraph stay) and do nothing when nothing would
    /// change; Toggle the task over a selection checks them all, then
    /// unchecks them all, and with the caret alone flips its own line.
    #[test]
    fn tasks_are_checked_and_unchecked_over_a_selection() {
        let text = "- [ ] a\n- [x] b\n- plain\n- [ ] c\n\nnote\n";
        let checked = "- [x] a\n- [x] b\n- plain\n- [x] c\n\nnote\n";
        let unchecked = "- [ ] a\n- [ ] b\n- plain\n- [ ] c\n\nnote\n";
        let all = Selection { anchor: 0, head: text.len() - 1 };
        let mut ed = selected(text, all.anchor, all.head);
        assert_eq!(edit(&mut ed, "list.check_tasks", 0, 10), Some(true));
        assert_eq!(ed.text(), checked);
        assert_eq!(edit(&mut ed, "list.check_tasks", 0, 10), Some(false), "all checked already");
        assert_eq!(edit(&mut ed, "list.uncheck_tasks", 0, 10), Some(true));
        assert_eq!(ed.text(), unchecked);
        assert_eq!(edit(&mut ed, "list.uncheck_tasks", 0, 10), Some(false), "none checked");
        assert_eq!(ed.selection(), all, "a box is as long either way: the selection stays");
        assert_eq!(edit(&mut ed, "list.toggle_task", 0, 10), Some(true));
        assert_eq!(ed.text(), checked);
        assert_eq!(edit(&mut ed, "list.toggle_task", 0, 10), Some(true));
        assert_eq!(ed.text(), unchecked);
        ed.set_caret("- [ ] a\n- [ ] b\n- pl".len(), false);
        assert_eq!(edit(&mut ed, "list.toggle_task", 0, 10), Some(true));
        assert_eq!(ed.text(), "- [ ] a\n- [ ] b\n- [ ] plain\n- [ ] c\n\nnote\n", "a plain item gets a box");
        // A selection with no task in it works on the caret's line.
        let mut ed = selected("- a\n- b\n", 0, "- a\n- b".len());
        assert_eq!(edit(&mut ed, "list.toggle_task", 0, 10), Some(true));
        assert_eq!(ed.text(), "- a\n- [ ] b\n");
    }

    /// 10.7: Bulleted, numbered and task list make the selected lines that
    /// list (from another kind too); the same kind again makes them plain.
    #[test]
    fn lines_become_each_kind_of_list_and_plain_again() {
        let text = "a\nb\n";
        let mut ed = selected(text, 0, "a\nb".len());
        assert_eq!(edit(&mut ed, "list.bullet", 0, 10), Some(true));
        assert_eq!(ed.text(), "- a\n- b\n");
        assert_eq!(edit(&mut ed, "list.number", 0, 10), Some(true));
        assert_eq!(ed.text(), "1. a\n2. b\n");
        assert_eq!(edit(&mut ed, "list.task", 0, 10), Some(true));
        assert_eq!(ed.text(), "- [ ] a\n- [ ] b\n");
        assert_eq!(edit(&mut ed, "list.task", 0, 10), Some(true));
        assert_eq!(ed.text(), text);
    }

    /// 10.8: A CRLF file stays CRLF through the format commands, inline and
    /// by line.
    #[test]
    fn format_commands_keep_crlf() {
        let text = "- a\r\n- b\r\n";
        let mut ed = selected(text, 0, "- a\r\n- b".len());
        assert_eq!(edit(&mut ed, "inline.bold", 0, 10), Some(true));
        assert_eq!(ed.text(), "- **a**\r\n- **b**\r\n");
        assert_eq!(edit(&mut ed, "edit.undo", 0, 10), Some(true));
        assert_eq!(edit(&mut ed, "list.number", 0, 10), Some(true));
        assert_eq!(ed.text(), "1. a\r\n2. b\r\n");
        assert_eq!(edit(&mut ed, "edit.undo", 0, 10), Some(true));
        assert_eq!(edit(&mut ed, "block.fence", 0, 10), Some(true));
        assert_eq!(ed.text(), "```\r\n- a\r\n- b\r\n```\r\n");
        assert_eq!(edit(&mut ed, "block.fence", 0, 10), Some(true));
        assert_eq!(ed.text(), text);
        assert_eq!(ed.eol(), ito_textbuf::Eol::Crlf);
    }

    /// 10.9: Inside a code block the format commands change nothing and leave
    /// no undo step behind; on a read-only file they change nothing either.
    #[test]
    fn nothing_happens_inside_code_or_on_a_read_only_file() {
        let text = "```\n- [ ] foo\n```\n";
        let mut ed = Editor::new(text);
        ed.set_caret("```\n- [ ] f".len(), false);
        for name in ["inline.bold", "inline.code", "heading.set_1", "heading.clear", "block.quote", "list.bullet", "list.check_tasks"] {
            assert_eq!(edit(&mut ed, name, 0, 10), Some(false), "{name}");
            assert_eq!(ed.text(), text, "{name}");
        }
        assert_eq!(edit(&mut ed, "edit.undo", 0, 10), Some(false), "no empty step");

        let dir = std::env::temp_dir().join(format!("weft-format-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bad = dir.join("sjis.md");
        std::fs::write(&bad, b"\x82\xa0abc\n").unwrap();
        let mut ed = Editor::open(&bad).unwrap();
        let read = ed.text().to_owned();
        ed.select_all();
        for name in ["inline.bold", "heading.set_1", "block.fence", "list.task", "list.toggle_task"] {
            assert_eq!(edit(&mut ed, name, 0, 10), Some(false), "{name}");
            assert_eq!(ed.text(), read, "{name}");
        }
        assert!(!ed.take_unflanked());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 10.10: Bold that lands next to punctuation (`**「強調」**です`) goes in,
    /// and the editor raises the notice for the app once; taking it off
    /// raises none.
    #[test]
    fn bold_next_to_punctuation_raises_the_notice_once() {
        let mut ed = selected("「強調」です", 0, "「強調」".len());
        assert_eq!(edit(&mut ed, "inline.bold", 0, 10), Some(true));
        assert_eq!(ed.text(), "**「強調」**です");
        assert!(ed.take_unflanked());
        assert!(!ed.take_unflanked(), "asking clears it");
        assert_eq!(edit(&mut ed, "inline.bold", 0, 10), Some(true));
        assert_eq!(ed.text(), "「強調」です");
        assert!(!ed.take_unflanked());
        let mut ed = selected("a 「強調」 b", 2, "a 「強調」".len());
        edit(&mut ed, "inline.bold", 0, 10);
        assert_eq!(ed.text(), "a **「強調」** b");
        assert!(!ed.take_unflanked(), "with spaces around it shows");
    }
}
