//! Every command weft runs, by name ("object.action"). The keymap, the
//! palette and the key help all take their names and descriptions from
//! [`ALL`], so a command is added here once. [`edit`] runs the ones that
//! work on the document; it has no UI, so the tests, `--keys` and the keys
//! reach the same code.

use std::ops::Range;

use crate::engine::{Editor, Motion};
use ito_mdedit::{Dir, HeadingDelete, Inline, Line, SelChange, Step, Target};

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
    // The document tabs: ito-tabs' names, run by `App` (src/tabs.rs).
    app(ito_tabs::cmd::NEXT, "Next tab"),
    app(ito_tabs::cmd::PREV, "Previous tab"),
    app(ito_tabs::cmd::CLOSE, "Close the tab"),
    app(ito_tabs::cmd::GOTO[0], "Go to tab 1"),
    app(ito_tabs::cmd::GOTO[1], "Go to tab 2"),
    app(ito_tabs::cmd::GOTO[2], "Go to tab 3"),
    app(ito_tabs::cmd::GOTO[3], "Go to tab 4"),
    app(ito_tabs::cmd::GOTO[4], "Go to tab 5"),
    app(ito_tabs::cmd::GOTO[5], "Go to tab 6"),
    app(ito_tabs::cmd::GOTO[6], "Go to tab 7"),
    app(ito_tabs::cmd::GOTO[7], "Go to tab 8"),
    app(ito_tabs::cmd::GOTO[8], "Go to tab 9"),
    app("view.toggle_preview", "Toggle the preview"),
    app("edit.paste_image", "Paste the clipboard picture"),
    app("view.toggle_wrap", "Toggle word wrap"),
    // Find and replace: ito-find's names, run by `App` (src/find.rs).
    app(ito_find::cmd::OPEN, "Find"),
    app(ito_find::cmd::REPLACE_OPEN, "Find and replace"),
    app(ito_find::cmd::NEXT, "Next match"),
    app(ito_find::cmd::PREV, "Previous match"),
    app(ito_find::cmd::REPLACE_ONE, "Replace this match"),
    app("heading.fold", "Fold or unfold this section"),
    app("heading.fold_all", "Fold all sections"),
    app("heading.unfold_all", "Unfold all sections"),
    app("view.focus", "Show only this section"),
    app("link.follow", "Open the linked note"),
    app("vault.search", "Search every note"),
    app("vault.backlinks", "Show the notes that link here"),
    app("lint.next", "Next lint finding"),
    app("lint.prev", "Previous lint finding"),
    app("lint.fix_all", "Fix all lint findings"),
    app(ito_find::cmd::REPLACE_ALL, "Replace all"),
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
    doc("heading.insert_toc", "Insert or update the table of contents"),
    doc("select.expand", "Expand the selection"),
    doc("select.shrink", "Shrink the selection"),
    doc("heading.delete", "Delete the heading line, keeping its body"),
    doc("section.delete", "Delete the section with its subsections"),
    doc("heading.to_paragraph", "Turn this section's heading into a paragraph"),
    doc("heading.delete_lift", "Delete the heading and lift its subheadings"),
    // Moves by structure: in the palette, unlike the plain caret moves.
    doc("cursor.next_heading", "To the next heading"),
    doc("cursor.prev_heading", "To the previous heading"),
    doc("cursor.next_list", "To the next list"),
    doc("cursor.prev_list", "To the previous list"),
    doc("cursor.next_table", "To the next table"),
    doc("cursor.prev_table", "To the previous table"),
    doc("cursor.next_code", "To the next code block"),
    doc("cursor.prev_code", "To the previous code block"),
    doc("cursor.next_task", "To the next open task"),
    doc("cursor.prev_task", "To the previous open task"),
    doc("cursor.parent_heading", "To the parent heading"),
    doc("cursor.child_heading", "To the first subheading"),
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
        // A new table of contents goes after the block at the caret; one
        // already there (between `<!-- toc -->` markers) is brought up to date.
        "heading.insert_toc" => ed.structure(ito_mdedit::insert_toc),
        "select.expand" => ed.expand_selection(),
        "select.shrink" => ed.shrink_selection(),
        // The heading of the section the caret is in, from anywhere in it.
        "heading.delete" => ed.structure(|t, b| ito_mdedit::delete_heading(t, b, HeadingDelete::LineOnly)),
        "section.delete" => ed.structure(|t, b| ito_mdedit::delete_heading(t, b, HeadingDelete::Section)),
        "heading.to_paragraph" => ed.structure(|t, b| ito_mdedit::delete_heading(t, b, HeadingDelete::ToParagraph)),
        "heading.delete_lift" => ed.structure(|t, b| ito_mdedit::delete_heading(t, b, HeadingDelete::LiftChildren)),
        "cursor.next_heading" => ed.jump(|t, b| ito_mdedit::next(t, b, Target::Heading, Dir::Down)),
        "cursor.prev_heading" => ed.jump(|t, b| ito_mdedit::next(t, b, Target::Heading, Dir::Up)),
        "cursor.next_list" => ed.jump(|t, b| ito_mdedit::next(t, b, Target::List, Dir::Down)),
        "cursor.prev_list" => ed.jump(|t, b| ito_mdedit::next(t, b, Target::List, Dir::Up)),
        "cursor.next_table" => ed.jump(|t, b| ito_mdedit::next(t, b, Target::Table, Dir::Down)),
        "cursor.prev_table" => ed.jump(|t, b| ito_mdedit::next(t, b, Target::Table, Dir::Up)),
        "cursor.next_code" => ed.jump(|t, b| ito_mdedit::next(t, b, Target::Code, Dir::Down)),
        "cursor.prev_code" => ed.jump(|t, b| ito_mdedit::next(t, b, Target::Code, Dir::Up)),
        "cursor.next_task" => ed.jump(|t, b| ito_mdedit::next(t, b, Target::OpenTask, Dir::Down)),
        "cursor.prev_task" => ed.jump(|t, b| ito_mdedit::next(t, b, Target::OpenTask, Dir::Up)),
        // The parent and the child have no direction.
        "cursor.parent_heading" => ed.jump(|t, b| ito_mdedit::next(t, b, Target::Parent, Dir::Up)),
        "cursor.child_heading" => ed.jump(|t, b| ito_mdedit::next(t, b, Target::Child, Dir::Down)),
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

    /// 8.24: Tab over several lines takes each one level in: a list item as
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

        // Pressed again where Shift+Tab could not give the text back (the
        // items would be code under the paragraph), it does nothing: ito's
        // `shift_lines` refuses a Tab that cannot be undone.
        let t = "para\n- a\n- b";
        let mut ed = at(t, 0);
        edit(&mut ed, "select.all", 0, 10);
        edit(&mut ed, "edit.tab", 0, 10);
        let once = ed.text().to_string();
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(false));
        assert_eq!(ed.text(), once);
        assert_eq!(ed.selected_text().as_deref(), Some(ed.text()));
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(true));
        assert_eq!(ed.text(), t);
    }

    /// 8.24: With a parent item and its child both selected, the parent goes
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

    /// 8.24: In a code fence every line is code: the fence lines and a `- x`
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

    /// 8.24: A table's rows do not go in or out on their own: only with a
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

    /// 8.24: CRLF lines keep their CRLF, items and other lines alike.
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

    /// 8.24: One press over several lines is one undo step: the text and the
    /// selection come back as they were, one press at a time.
    #[test]
    fn tab_over_lines_is_one_undo_step_with_the_selection() {
        let t = "x\npara\n- a\n- b\ny\n";
        let (from, to) = (t.find("ra").unwrap(), t.find("b\n").unwrap());
        let mut ed = select(t, from, to);
        let once = "x\n    para\n  - a\n  - b\ny\n";
        edit(&mut ed, "edit.tab", 0, 10);
        assert_eq!(ed.text(), once);
        assert_eq!(ed.selected_text().as_deref(), Some("ra\n  - a\n  - "));
        // Shift+Tab is a press of its own, with its own undo step.
        assert_eq!(edit(&mut ed, "edit.untab", 10, 10), Some(true));
        assert_eq!(ed.text(), t);
        assert_eq!(ed.selected_text().as_deref(), Some("ra\n- a\n- "));
        assert_eq!(edit(&mut ed, "edit.undo", 20, 10), Some(true));
        assert_eq!(ed.text(), once, "Shift+Tab too");
        assert_eq!(ed.selected_text().as_deref(), Some("ra\n  - a\n  - "));
        assert_eq!(edit(&mut ed, "edit.undo", 30, 10), Some(true));
        assert_eq!(ed.text(), t);
        assert_eq!(ed.selected_text().as_deref(), Some("ra\n- a\n- "));
        assert_eq!(edit(&mut ed, "edit.redo", 40, 10), Some(true));
        assert_eq!(ed.text(), once);
        assert_eq!(ed.selected_text().as_deref(), Some("ra\n  - a\n  - "));
    }

    /// 8.25: With the tab character on (the settings), Tab puts in a tab and
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

    /// The selection as `(anchor, head)`.
    fn ends(ed: &Editor) -> (usize, usize) {
        let s = ed.selection();
        (s.anchor, s.head)
    }

    /// 8.26: A list item, an empty line and a paragraph, all selected: Tab
    /// takes the item a level in and the paragraph by the same amount, so it
    /// stands where it stood against the item; the empty line stays empty.
    /// Shift+Tab gives the text back, and twice in then twice out does too.
    #[test]
    fn tab_over_an_item_an_empty_line_and_a_paragraph_round_trips() {
        let t = "- a\n\npara\n";
        let mut ed = at(t, 0);
        edit(&mut ed, "select.all", 0, 10);
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!(ed.text(), "  - a\n\n  para\n");
        assert_eq!(ed.selected_text().as_deref(), Some(ed.text()));
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(true));
        assert_eq!(ed.text(), t);
        assert_eq!(ed.selected_text().as_deref(), Some(t));
        for k in ["edit.tab", "edit.tab", "edit.untab", "edit.untab"] {
            edit(&mut ed, k, 0, 10);
        }
        assert_eq!(ed.text(), t, "twice in, twice out");
        assert_eq!(ed.selected_text().as_deref(), Some(t));
    }

    /// 8.27: Eleven numbered items, all selected: Tab takes them in together,
    /// `9.` and `10.` in line, and the numbers stay; Shift+Tab gives the text
    /// back, and twice in then twice out does too. With the caret on `10.`
    /// alone, Tab makes it the first of a sublist under `9.` and the list it
    /// left goes on numbered; Shift+Tab puts it back.
    #[test]
    fn tab_over_numbered_items_past_nine_round_trips_with_the_numbers() {
        let t: String = (1..=11).map(|i| format!("{i}. x{i}\n")).collect();
        let mut ed = at(&t, 0);
        edit(&mut ed, "select.all", 0, 10);
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        let indented: String = (1..=11).map(|i| format!("   {i}. x{i}\n")).collect();
        assert_eq!(ed.text(), indented, "one amount for the run, so the items stay in line");
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(true));
        assert_eq!(ed.text(), t);
        for k in ["edit.tab", "edit.tab", "edit.untab", "edit.untab"] {
            edit(&mut ed, k, 0, 10);
        }
        assert_eq!(ed.text(), t, "twice in, twice out");
        assert_eq!(ed.selected_text().as_deref(), Some(t.as_str()));

        let t = "9. x9\n10. x10\n11. x11\n";
        let mut ed = at(t, t.find("x10").unwrap());
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!(ed.text(), "9. x9\n   1. x10\n10. x11\n");
        assert_eq!(&ed.text()[ed.caret()..], "x10\n10. x11\n", "the caret stays on its word");
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(true));
        assert_eq!(ed.text(), t);
        assert_eq!(&ed.text()[ed.caret()..], "x10\n11. x11\n");
    }

    /// 8.28: A selection that ends inside a code fence moves the whole fence,
    /// to its closing line: the lines after it stay where they are and are not
    /// code, and a `- x` in the fence is still code. Shift+Tab gives the text
    /// back. A fence in a list item moves whole with the item.
    #[test]
    fn tab_over_a_selection_ending_in_a_fence_moves_the_whole_fence() {
        let is_item = |text: &str, line: &str| ito_mdedit::indent_item(text, text.find(line).unwrap()).is_some();
        let t = "para\n```\ncode\n- x\n```\n- after\n\nend\n";
        let mut ed = select(t, 0, t.find("ode").unwrap());
        ed.set_tab_width(2);
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!(ed.text(), "  para\n  ```\n  code\n  - x\n  ```\n- after\n\nend\n");
        assert_eq!(ed.selected_text().as_deref(), Some("  para\n  ```\n  c"), "the selection stays on its text");
        assert!(is_item(ed.text(), "- after"), "the fence is closed: the item after it is not code");
        assert!(!is_item(ed.text(), "- x"), "the line in the fence is still code");
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(true));
        assert_eq!(ed.text(), t);
        assert_eq!(ed.selected_text().as_deref(), Some("para\n```\nc"));

        // With the tab width 4 too, the closing line goes with the fence.
        let mut ed = select(t, 0, t.find("ode").unwrap());
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!(ed.text(), "    para\n    ```\n    code\n    - x\n    ```\n- after\n\nend\n");
        assert!(is_item(ed.text(), "- after"));

        let t = "- item\n\n  ```\n  code\n  ```\n- after\n";
        for width in [2, 4] {
            let mut ed = select(t, 0, t.find("ode").unwrap());
            ed.set_tab_width(width);
            assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
            assert_eq!(ed.text(), "  - item\n\n    ```\n    code\n    ```\n- after\n", "width {width}");
            assert!(is_item(ed.text(), "- after"));
            assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(true));
            assert_eq!(ed.text(), t, "width {width}");
        }
    }

    /// 8.29: With the tab character on, in a CRLF file, over a selection made
    /// backwards: a paragraph line gets one tab, the item goes a level in as an
    /// item (in spaces) with the line under it, every line keeps its CRLF, and
    /// the selection stays backwards over the same lines. One Ctrl+Z takes
    /// back one whole press, with its selection.
    #[test]
    fn tab_with_the_tab_character_over_crlf_lines_keeps_crlf_and_the_selection_backwards() {
        let t = "para\r\n- a\r\n  - b\r\nend\r\n";
        let once = "\tpara\r\n  - a\r\n    - b\r\n  end\r\n";
        let mut ed = select(t, t.len(), 0);
        ed.set_tab_char(true);
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!(ed.text(), once, "the line under the item goes with it, by its amount");
        assert_eq!(ends(&ed), (once.len(), 0), "still backwards, over every line");
        // A second Tab would be one Shift+Tab could not give back (ito's
        // `shift_lines` refuses it): nothing moves and nothing is recorded.
        assert_eq!(edit(&mut ed, "edit.tab", 10, 10), Some(false));
        assert_eq!((ed.text(), ends(&ed)), (once, (once.len(), 0)));
        assert_eq!(edit(&mut ed, "edit.untab", 20, 10), Some(true));
        assert_eq!((ed.text(), ends(&ed)), (t, (t.len(), 0)));
        assert_eq!(edit(&mut ed, "edit.undo", 30, 10), Some(true));
        assert_eq!((ed.text(), ends(&ed)), (once, (once.len(), 0)), "one Ctrl+Z, one press");
        assert_eq!(edit(&mut ed, "edit.undo", 40, 10), Some(true));
        assert_eq!((ed.text(), ends(&ed)), (t, (t.len(), 0)), "the refused Tab left nothing to take back");

        let t = "para\r\nmore\r\n";
        let mut ed = at(t, 0);
        ed.set_tab_char(true);
        edit(&mut ed, "select.all", 0, 10);
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(true));
        assert_eq!(edit(&mut ed, "edit.tab", 10, 10), Some(true));
        assert_eq!(ed.text(), "\t\tpara\r\n\t\tmore\r\n");
        assert_eq!(edit(&mut ed, "edit.untab", 20, 10), Some(true));
        assert_eq!(ed.text(), "\tpara\r\n\tmore\r\n", "one tab out a press");
        assert_eq!(edit(&mut ed, "edit.undo", 30, 10), Some(true));
        assert_eq!(ed.text(), "\t\tpara\r\n\t\tmore\r\n");
    }

    /// 8.30: Over several lines, Tab and Shift+Tab change nothing in a
    /// read-only file, nor where nothing can move (empty lines only; for
    /// Shift+Tab, lines and items at the left edge), and leave nothing to undo.
    #[test]
    fn tab_over_lines_that_cannot_move_changes_nothing() {
        let dir = std::env::temp_dir().join(format!("weft-tab-lines-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("lines.md");
        std::fs::write(&path, b"para\n- a\n  - b\n\x82\xa0\n").unwrap();
        let mut ed = Editor::open(&path).unwrap();
        assert!(ed.locked().is_some());
        let text = ed.text().to_owned();
        edit(&mut ed, "select.all", 0, 10);
        assert_eq!(edit(&mut ed, "edit.tab", 0, 10), Some(false));
        assert_eq!(edit(&mut ed, "edit.untab", 0, 10), Some(false));
        assert_eq!(ed.text(), text);
        assert_eq!(edit(&mut ed, "edit.undo", 0, 10), Some(false), "nothing was done");
        let _ = std::fs::remove_dir_all(&dir);

        for (t, tab) in [("\n\n\n", true), ("\n\n\n", false), ("a\nb\n", false), ("- a\n- b\n", false)] {
            let mut ed = at(t, 0);
            edit(&mut ed, "select.all", 0, 10);
            let k = if tab { "edit.tab" } else { "edit.untab" };
            assert_eq!(edit(&mut ed, k, 0, 10), Some(false), "{k} on {t:?}");
            assert_eq!(ed.text(), t);
            assert_eq!(ed.selected_text().as_deref(), Some(t));
            assert_eq!(edit(&mut ed, "edit.undo", 0, 10), Some(false), "{k} on {t:?}: nothing to undo");
        }
    }

    /// Not a check, a measure: select-all Tab, Shift+Tab, and Shift+Tab where
    /// nothing moves, over about 1 MB that ends in 1,000 list items. Run it in
    /// release: `cargo test --release -p weft tab_over_a_megabyte -- --ignored --nocapture`.
    #[test]
    #[ignore = "a timing to read by hand, in release"]
    fn tab_over_a_megabyte_timing() {
        use std::time::{Duration, Instant};
        let mut head = String::new();
        let mut n = 0;
        while head.len() < 1_000_000 {
            n += 1;
            head += &format!("## Part {n}\n\nSome words in a paragraph, to make the file big.\nAnd a second line of it.\n\n");
        }
        for (name, bullet) in [("- item", true), ("N. item", false)] {
            let item = |i: usize| if bullet { format!("- item {i}\n") } else { format!("{i}. item\n") };
            let t: String = head.clone() + &(1..=1000).map(item).collect::<String>();
            let mut ed = at(&t, 0);
            edit(&mut ed, "select.all", 0, 10);
            let mut time = |k: &str, want: bool| {
                let start = Instant::now();
                assert_eq!(edit(&mut ed, k, 0, 10), Some(want), "{name} {k}");
                start.elapsed()
            };
            let (mut tab, mut untab, mut none) = (Vec::new(), Vec::new(), Vec::new());
            for _ in 0..5 {
                tab.push(time("edit.tab", true));
                untab.push(time("edit.untab", true));
                none.push(time("edit.untab", false));
            }
            assert_eq!(ed.text(), t);
            let median = |v: &mut Vec<Duration>| {
                v.sort();
                v[v.len() / 2]
            };
            println!(
                "{name}: {} bytes, Tab {:?}, Shift+Tab {:?}, Shift+Tab with nothing to move {:?} (medians of 5)",
                t.len(),
                median(&mut tab),
                median(&mut untab),
                median(&mut none)
            );
        }
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

    /// The names of the new structure commands, for the checks they share.
    const TOC_AND_DELETES: [&str; 5] =
        ["heading.insert_toc", "heading.delete", "section.delete", "heading.to_paragraph", "heading.delete_lift"];

    /// 11.1: Table of contents puts one after the block at the caret, the
    /// caret on its first line, as one undo step. Run again (from anywhere)
    /// after a heading is added, it brings that list up to date rather than
    /// adding another; with nothing to change it does nothing.
    #[test]
    fn a_table_of_contents_goes_in_and_is_brought_up_to_date() {
        let text = "# Doc\n\nintro\n\n## A\n\n### A1\n\n## B\n\nend\n";
        let toc = "# Doc\n\nintro\n\n<!-- toc -->\n- [Doc](#doc)\n  - [A](#a)\n    - [A1](#a1)\n  - [B](#b)\n<!-- /toc -->\n\n\
                   ## A\n\n### A1\n\n## B\n\nend\n";
        let caret = "# Doc\n\nin".len();
        let mut ed = Editor::new(text);
        ed.set_caret(caret, false);
        assert_eq!(edit(&mut ed, "heading.insert_toc", 0, 10), Some(true));
        assert_eq!(ed.text(), toc);
        assert!(ed.text()[ed.caret()..].starts_with("<!-- toc -->"), "the caret goes to the list");
        assert_eq!(edit(&mut ed, "edit.undo", 0, 10), Some(true));
        assert_eq!(ed.text(), text);
        assert_eq!(ed.caret(), caret);
        assert_eq!(edit(&mut ed, "edit.undo", 0, 10), Some(false), "it was one step");
        assert_eq!(edit(&mut ed, "edit.redo", 0, 10), Some(true));
        assert_eq!(ed.text(), toc);

        // A heading added at the end leaves the list behind.
        ed.set_caret(toc.len(), false);
        assert!(ed.insert("\n## C\n", 0));
        let stale = ed.text().to_owned();
        ed.set_caret(0, false);
        assert_eq!(edit(&mut ed, "heading.insert_toc", 0, 10), Some(true));
        assert!(ed.text().contains("  - [B](#b)\n  - [C](#c)\n<!-- /toc -->\n"), "{}", ed.text());
        assert_eq!(ed.text().matches("<!-- toc -->").count(), 1, "brought up to date, not added again");
        assert_eq!(edit(&mut ed, "heading.insert_toc", 0, 10), Some(false), "up to date already");
        assert_eq!(edit(&mut ed, "edit.undo", 0, 10), Some(true));
        assert_eq!(ed.text(), stale);

        let mut ed = Editor::new("no headings\n");
        assert_eq!(edit(&mut ed, "heading.insert_toc", 0, 10), Some(false));
        assert_eq!(edit(&mut ed, "edit.undo", 0, 10), Some(false), "no empty step");
    }

    const SENTENCES: &str = "# One\n\nHello world. Second one.\n\n## Two\n\ntail\n";

    /// 11.2: Expand the selection from a caret in a word grows it step by
    /// step (the word, the sentence, the paragraph, the section, the whole
    /// text), the caret at the end; Shrink the selection goes back the same
    /// steps to the caret where it was. A selection made backwards grows
    /// and shrinks backwards. Neither is an undo step.
    #[test]
    fn expand_then_shrink_goes_back_the_same_way() {
        let mut ed = Editor::new(SENTENCES);
        let start = SENTENCES.find("world").unwrap() + 2;
        ed.set_caret(start, false);
        let steps = ["world", "Hello world.", "Hello world. Second one.", "# One\n\nHello world. Second one.\n\n## Two\n\ntail", SENTENCES];
        let mut grown = Vec::new();
        for want in steps {
            assert_eq!(edit(&mut ed, "select.expand", 0, 10), Some(true), "to {want:?}");
            assert_eq!(ed.selected_text().as_deref(), Some(want));
            let sel = ed.selection();
            assert!(sel.anchor < sel.head, "the caret at the end: {sel:?}");
            grown.push(sel);
        }
        assert_eq!(edit(&mut ed, "select.expand", 0, 10), Some(false), "the whole text already");
        grown.pop();
        while let Some(want) = grown.pop() {
            assert_eq!(edit(&mut ed, "select.shrink", 0, 10), Some(true));
            assert_eq!(ed.selection(), want);
        }
        assert_eq!(edit(&mut ed, "select.shrink", 0, 10), Some(true));
        assert_eq!(ed.selection(), Selection { anchor: start, head: start }, "back to the caret");
        assert_eq!(edit(&mut ed, "select.shrink", 0, 10), Some(false), "nothing smaller than a caret");
        assert_eq!(ed.text(), SENTENCES);
        assert_eq!(edit(&mut ed, "edit.undo", 0, 10), Some(false), "no undo step");

        // "wor" selected backwards: the caret stays at the start.
        let w = SENTENCES.find("world").unwrap();
        let back = Selection { anchor: w + 3, head: w };
        let mut ed = selected(SENTENCES, back.anchor, back.head);
        assert_eq!(edit(&mut ed, "select.expand", 0, 10), Some(true));
        assert_eq!(ed.selection(), Selection { anchor: w + 5, head: w });
        assert_eq!(edit(&mut ed, "select.expand", 0, 10), Some(true));
        assert_eq!(ed.selected_text().as_deref(), Some("Hello world."));
        assert!(ed.selection().head < ed.selection().anchor);
        edit(&mut ed, "select.shrink", 0, 10);
        assert_eq!(edit(&mut ed, "select.shrink", 0, 10), Some(true));
        assert_eq!(ed.selection(), back, "exactly as it was");
    }

    /// 11.3: The way back is dropped when the text changes (an edit, an
    /// undo, even one that gives the same text back) or the selection is
    /// moved: Shrink the selection then takes the largest structure inside
    /// the selection that starts where it starts.
    #[test]
    fn the_way_back_is_dropped_after_an_edit_or_a_move() {
        let section = Selection { anchor: 0, head: "# One\n\nHello world. Second one.\n\n## Two\n\ntail".len() };
        let grow = |ed: &mut Editor| {
            ed.set_caret(SENTENCES.find("world").unwrap() + 2, false);
            for _ in 0..4 {
                assert_eq!(edit(ed, "select.expand", 0, 10), Some(true));
            }
            assert_eq!(ed.selection(), section);
        };
        let back_to = |ed: &mut Editor, sel: Selection| {
            ed.set_caret(sel.anchor, false);
            ed.set_caret(sel.head, true);
        };

        // Untouched, Shrink goes back to the paragraph it grew from.
        let mut ed = Editor::new(SENTENCES);
        grow(&mut ed);
        assert_eq!(edit(&mut ed, "select.shrink", 0, 10), Some(true));
        assert_eq!(ed.selected_text().as_deref(), Some("Hello world. Second one."));

        // Typed at the end, then the same selection again.
        let mut ed = Editor::new(SENTENCES);
        grow(&mut ed);
        ed.set_caret(SENTENCES.len(), false);
        assert!(ed.insert("!", 0));
        back_to(&mut ed, section);
        assert_eq!(edit(&mut ed, "select.shrink", 0, 10), Some(true));
        assert_eq!(ed.selected_text().as_deref(), Some("# One"), "not the paragraph it grew from");

        // Typed and undone: the same text, but not the same version.
        let mut ed = Editor::new(SENTENCES);
        grow(&mut ed);
        ed.set_caret(SENTENCES.len(), false);
        assert!(ed.insert("!", 0));
        assert_eq!(edit(&mut ed, "edit.undo", 0, 10), Some(true));
        assert_eq!(ed.text(), SENTENCES);
        back_to(&mut ed, section);
        assert_eq!(edit(&mut ed, "select.shrink", 0, 10), Some(true));
        assert_eq!(ed.selected_text().as_deref(), Some("# One"));

        // Moved: one more character selected, then shrunk twice.
        let mut ed = Editor::new(SENTENCES);
        grow(&mut ed);
        assert_eq!(edit(&mut ed, "select.right", 0, 10), Some(true));
        assert_eq!(ed.selected_text().as_deref(), Some(SENTENCES));
        assert_eq!(edit(&mut ed, "select.shrink", 0, 10), Some(true));
        assert_eq!(ed.selection(), section, "the section, the largest thing that starts at the start");
        assert_eq!(edit(&mut ed, "select.shrink", 0, 10), Some(true));
        assert_eq!(ed.selected_text().as_deref(), Some("# One"));

        let mut ed = Editor::new(SENTENCES);
        ed.set_caret(3, false);
        assert_eq!(edit(&mut ed, "select.shrink", 0, 10), Some(false), "a bare caret");
    }

    const SECTIONS: &str = "# A\n\nintro\n\n## B\n\nbody\n\n### C\n\nc\n\n## D\n\nd\n";

    /// 11.4: The four ways to delete a heading, run from the body of its
    /// section: the line alone, the section with its subsections, the
    /// heading made a paragraph, and the heading gone with its subheadings a
    /// level up. Each is one undo step; before the first heading none of
    /// them does anything.
    #[test]
    fn a_heading_is_deleted_four_ways_each_one_undo() {
        let body = SECTIONS.find("body").unwrap();
        for (name, want) in [
            ("heading.delete", "# A\n\nintro\n\nbody\n\n### C\n\nc\n\n## D\n\nd\n"),
            ("section.delete", "# A\n\nintro\n\n## D\n\nd\n"),
            ("heading.to_paragraph", "# A\n\nintro\n\nB\n\nbody\n\n### C\n\nc\n\n## D\n\nd\n"),
            ("heading.delete_lift", "# A\n\nintro\n\nbody\n\n## C\n\nc\n\n## D\n\nd\n"),
        ] {
            let mut ed = Editor::new(SECTIONS);
            ed.set_caret(body, false);
            assert_eq!(edit(&mut ed, name, 0, 10), Some(true), "{name}");
            assert_eq!(ed.text(), want, "{name}");
            assert_eq!(edit(&mut ed, "edit.undo", 0, 10), Some(true), "{name}");
            assert_eq!(ed.text(), SECTIONS, "{name}: one undo gives it all back");
            assert_eq!(ed.caret(), body, "{name}");
            assert_eq!(edit(&mut ed, "edit.undo", 0, 10), Some(false), "{name}: it was one step");
            assert_eq!(edit(&mut ed, "edit.redo", 0, 10), Some(true), "{name}");
            assert_eq!(ed.text(), want, "{name}");
        }
        for name in TOC_AND_DELETES.into_iter().skip(1) {
            let mut ed = Editor::new("text\n\n# A\n");
            ed.set_caret(1, false);
            assert_eq!(edit(&mut ed, name, 0, 10), Some(false), "{name}: no heading above");
            assert_eq!(ed.text(), "text\n\n# A\n");
        }
    }

    const PLACES: &str = "# One\n\ntext\n\n- [x] done\n- [ ] todo\n  - [ ] nested\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n\
                          ```rust\nfn main() {}\n```\n\n## 二\n\n- plain\n\n### Three\n\n# Four\n";

    /// 11.5: The moves to the next and previous heading, list, table, code
    /// block and open task, and to the parent heading and the first
    /// subheading, put the caret at the start of the one they find (the
    /// selection dropped) and change no text; with nothing there the caret
    /// stays where it is.
    #[test]
    fn the_moves_go_to_the_next_thing_of_their_kind() {
        let at = |s: &str| PLACES.find(s).unwrap();
        let moves = [
            ("cursor.next_heading", 0, Some(at("## 二"))),
            ("cursor.prev_heading", 0, None),
            ("cursor.next_heading", at("# Four"), None),
            ("cursor.prev_heading", at("# Four"), Some(at("### Three"))),
            ("cursor.next_list", 0, Some(at("- [x]"))),
            ("cursor.prev_list", at("nested"), Some(at("- [x]"))),
            ("cursor.next_table", 0, Some(at("| a"))),
            ("cursor.prev_table", at("## 二"), Some(at("| a"))),
            ("cursor.next_code", 0, Some(at("```rust"))),
            ("cursor.next_code", at("```rust"), None),
            ("cursor.prev_code", at("# Four"), Some(at("```rust"))),
            ("cursor.next_task", 0, Some(at("- [ ] todo"))),
            ("cursor.next_task", at("- [ ] todo"), Some(at("- [ ] nested"))),
            ("cursor.prev_task", at("| a"), Some(at("- [ ] nested"))),
            ("cursor.parent_heading", at("- plain"), Some(at("## 二"))),
            ("cursor.parent_heading", at("## 二") + 2, Some(0)),
            ("cursor.parent_heading", 0, None),
            ("cursor.child_heading", at("text"), Some(at("## 二"))),
            ("cursor.child_heading", at("- plain"), Some(at("### Three"))),
            ("cursor.child_heading", at("# Four"), None),
        ];
        for (name, from, to) in moves {
            let mut ed = Editor::new(PLACES);
            ed.set_caret(from, false);
            let done = edit(&mut ed, name, 0, 10);
            match to {
                Some(to) => {
                    assert_eq!(done, Some(true), "{name} from {from}");
                    assert_eq!(ed.selection(), Selection { anchor: to, head: to }, "{name} from {from}");
                }
                None => {
                    assert_eq!(done, Some(false), "{name} from {from}");
                    assert_eq!(ed.selection(), Selection { anchor: from, head: from }, "{name} from {from}");
                }
            }
            assert_eq!(ed.text(), PLACES, "{name}");
            assert_eq!(edit(&mut ed, "edit.undo", 0, 10), Some(false), "{name}: no undo step");
        }
        let mut ed = selected(PLACES, 0, 3);
        assert_eq!(edit(&mut ed, "cursor.next_heading", 0, 10), Some(true));
        assert_eq!(ed.selection(), Selection { anchor: at("## 二"), head: at("## 二") }, "the selection is dropped");
    }

    /// 11.6: Run as the keys and the palette run them, a move sets the view
    /// to follow the caret, and a command with nothing to do at the caret is
    /// said to the screen reader, in the window's language.
    #[test]
    fn the_app_follows_the_caret_and_says_when_there_is_nothing_to_do() {
        use crate::keymap::Cmd;
        let ctx = egui::Context::default();
        let mut app = crate::App::for_tests();
        app.doc.editor = Some(Editor::new("text\n\n# A\n\nbody\n"));
        app.doc.view.follow = false;
        app.run_key(&ctx, Cmd::Run("cursor.next_heading"));
        assert_eq!(app.doc.editor.as_ref().unwrap().caret(), "text\n\n".len());
        assert!(app.doc.view.follow, "the view goes to the caret");
        assert!(app.said.is_empty(), "{:?}", app.said);

        app.doc.editor.as_mut().unwrap().set_caret(1, false);
        app.run_key(&ctx, Cmd::Run("section.delete"));
        assert_eq!(app.said, ["Delete the section with its subsections: nothing to do here"]);
        assert_eq!(app.doc.editor.as_ref().unwrap().text(), "text\n\n# A\n\nbody\n");
        app.table = crate::i18n::table("ja").0;
        app.run_key(&ctx, Cmd::Run("cursor.prev_heading"));
        assert_eq!(app.said[1], "前の見出しへ: ここでは何もできません");
        app.run_key(&ctx, Cmd::Run("select.shrink"));
        assert_eq!(app.said[2], "選択を構造に沿って狭める: ここでは何もできません");
    }

    /// 11.7: A CRLF file with a BOM keeps both: a table of contents and a
    /// setext heading made a paragraph come out CRLF throughout, and the
    /// save writes the BOM back.
    #[test]
    fn the_new_commands_keep_crlf_and_the_bom() {
        let mut ed = Editor::new("Two\r\nlines\r\n---\r\n\r\nx\r\n");
        ed.set_caret("Two\r\nlines\r\n---\r\n\r\n".len(), false);
        assert_eq!(edit(&mut ed, "heading.to_paragraph", 0, 10), Some(true));
        assert_eq!(ed.text(), "Two\r\nlines\r\n\r\nx\r\n");
        let mut ed = Editor::new(SECTIONS.replace('\n', "\r\n").as_str());
        ed.set_caret(SECTIONS.replace('\n', "\r\n").find("body").unwrap(), false);
        assert_eq!(edit(&mut ed, "heading.delete_lift", 0, 10), Some(true));
        assert_eq!(ed.text(), "# A\n\nintro\n\nbody\n\n## C\n\nc\n\n## D\n\nd\n".replace('\n', "\r\n"));

        let dir = std::env::temp_dir().join(format!("weft-struct-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("bom.md");
        std::fs::write(&file, "\u{feff}# A\r\n\r\ntext\r\n\r\n## B\r\n").unwrap();
        let mut ed = Editor::open(&file).unwrap();
        assert!(ed.bom());
        let caret = ed.text().find("text").unwrap() + 2;
        ed.set_caret(caret, false);
        assert_eq!(edit(&mut ed, "heading.insert_toc", 0, 10), Some(true));
        assert_eq!(ed.text(), "# A\r\n\r\ntext\r\n\r\n<!-- toc -->\r\n- [A](#a)\r\n  - [B](#b)\r\n<!-- /toc -->\r\n\r\n## B\r\n");
        assert_eq!(ed.eol(), ito_textbuf::Eol::Crlf);
        let saved = ed.save_job().unwrap().bytes;
        assert!(saved.starts_with('\u{feff}'), "{saved:?}");
        assert!(!saved.replace("\r\n", "").contains('\n'), "no bare LF: {saved:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 11.8: On a read-only file the table of contents and the deletes
    /// change nothing; the moves, Expand and Shrink the selection change
    /// only the selection, so they still work.
    #[test]
    fn a_read_only_file_is_not_changed_but_can_be_moved_in() {
        let dir = std::env::temp_dir().join(format!("weft-struct-ro-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bad = dir.join("sjis.md");
        std::fs::write(&bad, b"\x82\xa0\n\n# A\n\nbody\n\n## B\n\nb\n").unwrap();
        let mut ed = Editor::open(&bad).unwrap();
        assert!(ed.locked().is_some());
        let read = ed.text().to_owned();
        let body = read.find("body").unwrap();
        for name in TOC_AND_DELETES {
            ed.set_caret(body, false);
            assert_eq!(edit(&mut ed, name, 0, 10), Some(false), "{name}");
            assert_eq!(ed.text(), read, "{name}");
        }
        ed.set_caret(body + 1, false);
        assert_eq!(edit(&mut ed, "select.expand", 0, 10), Some(true));
        assert_eq!(ed.selected_text().as_deref(), Some("body"));
        assert_eq!(edit(&mut ed, "select.shrink", 0, 10), Some(true));
        assert_eq!(ed.selection(), Selection { anchor: body + 1, head: body + 1 });
        assert_eq!(edit(&mut ed, "cursor.next_heading", 0, 10), Some(true));
        assert_eq!(ed.caret(), read.find("## B").unwrap());
        assert_eq!(ed.text(), read);
        assert!(!ed.is_dirty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 11.9: Every new command is named in Japanese, and a key bound to one
    /// shows in the key help by that name. Expand and Shrink the selection
    /// never split a grapheme cluster: a sentence that ends in `!` with a
    /// combining mark on it takes the mark too, both ways.
    #[test]
    fn the_new_commands_have_japanese_names_and_keep_graphemes_whole() {
        let ja = crate::i18n::table("ja").0;
        let first = ALL.iter().position(|d| d.name == "heading.insert_toc").unwrap();
        let new = &ALL[first..];
        assert_eq!(new.len(), 19);
        for d in new {
            assert!(d.palette && d.layer == Layer::Edit, "{}", d.name);
            assert_ne!(ja.tr_desc(d.desc), d.desc, "{} has no Japanese name", d.name);
        }
        let mut app = crate::App::for_tests();
        app.table = ja;
        // As the Keys page writes it: in front of the built-in keys.
        let (keys, warnings) =
            crate::keymap::Keys::load(Some(("keymap.toml", "[[edit.prepend_keymap]]\non = \"<A-w>\"\nrun = \"select.expand\"\n")));
        assert!(warnings.is_empty(), "{warnings:?}");
        app.keys = keys;
        let rows: Vec<(String, String)> = crate::help::groups(&app).into_iter().flat_map(|g| g.rows).collect();
        assert!(rows.contains(&("Alt+W".to_owned(), "選択を構造に沿って広げる".to_owned())), "{rows:?}");

        let text = "Wow!\u{301} next.";
        let mut ed = Editor::new(text);
        ed.set_caret(1, false);
        assert_eq!(edit(&mut ed, "select.expand", 0, 10), Some(true));
        assert_eq!(ed.selected_text().as_deref(), Some("Wow"));
        assert_eq!(edit(&mut ed, "select.expand", 0, 10), Some(true));
        assert_eq!(ed.selected_text().as_deref(), Some("Wow!\u{301}"), "the mark goes with the `!`");
        assert_eq!(edit(&mut ed, "select.shrink", 0, 10), Some(true));
        assert_eq!(ed.selected_text().as_deref(), Some("Wow"));
        ed.select_all();
        assert_eq!(edit(&mut ed, "select.shrink", 0, 10), Some(true));
        assert_eq!(ed.selected_text().as_deref(), Some("Wow!\u{301}"), "the way down keeps it too");
    }
}
