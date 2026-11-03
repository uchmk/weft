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
