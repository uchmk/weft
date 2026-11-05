# Testing

The checks a person (or an agent with a screen) makes. Each section is `## N. Title` over a table of `# | Do | Expect`.
A test takes a row off the list by naming its number in a doc comment on the `#[test]` (`/// 1.1: …`), and
`cargo run --example make-testcheck` writes what is left to `TESTING-CHECKS.md`.

## 1. Start

| # | Do | Expect |
|---|---|---|
| 1.1 | Run `weft` | A window of 1280x800 opens with the title "weft" |
| 1.2 | Move and resize the window, close it and start it again | The window comes back at the same place and size |
| 1.3 | Press `F12` | The browser opens the repository's new issue form with the version and OS filled in |
| 1.4 | Look at the title bar | There is no system title bar; at the left the woven icon (three teal threads across, three grey threads up and down, going over and under) and "weft"; the palette field, the bell and the minimize, maximize and close buttons at the right |
| 1.5 | Run `weft` with no file | An empty-state ghost in the middle ("No file is open" over "Start weft with a Markdown file: weft FILE.md"), with no button under it |
| 1.6 | Look at `weft.exe` in Explorer and on the taskbar | The same woven icon as in the title bar |
| 1.7 | Press `Ctrl+Q` with nothing unsaved | The window closes; started again, it comes back at the place it closed at |
| 1.8 | Run `weft --keys`, then `weft --keys "<F1>" --more` | weft says `--keys needs a script`, then ``unknown argument `--more` ``, and does not start |

## 2. Key help

| # | Do | Expect |
|---|---|---|
| 2.1 | Press `F1` | A card over the dimmed window lists the keys in groups: "Everywhere" (settings, key help, quit, save, the external editor, undo and redo), "Editing" (the caret, the selection, typing and the structure commands), "Always" (F12, the palette, the scale) and "This list" |
| 2.2 | Press `↓`, `PgDn`, `End`, `Home`, and turn the wheel over the card (in a window small enough that the list scrolls) | The list scrolls a line, a page, to the end and back to the top; the wheel scrolls by lines |
| 2.3 | Press `Esc`; open the help again and press `F1`; open it again and click outside the card | Each time the help closes |
| 2.4 | Type `?` | Nothing opens: `?` is left for the text, only `F1` opens the help |
| 2.5 | Switch the language to English, then to Japanese, opening the help each time | Every row has its words in that language; no row shows a key name such as `keydesc.…` |

## 3. Settings

| # | Do | Expect |
|---|---|---|
| 3.1 | Press `Ctrl+,` | The settings open on the General page (language, size, clock) |
| 3.2 | Press `Ctrl+Tab` until the pages come round | General, Theme, Editing, Keys, Notifications, Advanced and About, in that order |
| 3.3 | On the Keys page, change the key of Quit to `Ctrl+W` | The row shows `Ctrl+W`; `Ctrl+W` in the window quits, `Ctrl+Q` no longer does |
| 3.4 | Open `keymap.toml` in weft's config folder after 3.3, then press ↺ on the row | Only the changed binding is written; after ↺ nothing of it is left |
| 3.5 | On the Keys page, give a binding `F12` | The page says weft reads `F12` itself, so the binding never runs |
| 3.6 | On the Notifications page, press the test button of "Could not save" with the window behind another one | A system notification "weft: test notification" arrives (and the sound, if one is chosen) |
| 3.7 | Change the theme on the Theme page | The window and the sample change colour at once; another uchmk app open at the same time follows within 2 seconds |
| 3.8 | Press `Esc` | The settings close and the window comes back |
| 3.9 | On the Editing page, pick 2 as the tab width, close the settings and press `Tab` in a paragraph, then `Tab` with the IME composing; open `config.toml` in weft's config folder | The page offers 2, 4 and 8 (4 at first); `Tab` puts in 2 spaces at once and the focus stays in the text (no button of the bar lights up); while composing the IME keeps the key; the file has `tab_width = 2` under `[editor]` and its other lines as they were |
| 3.10 | On the Editing page, turn on "Insert a tab character" (with the keys too), close the settings and press `Tab` in a paragraph; open `config.toml` in weft's config folder; turn it off again | The switch sits under the tab width, with its note; `Tab` puts in one tab character; the file has `tab_char = true` under `[editor]` beside `tab_width`, and its other lines as they were; turned off, it says `tab_char = false` and `Tab` puts in spaces again |

## 4. Command palette

| # | Do | Expect |
|---|---|---|
| 4.1 | Press `Ctrl+Shift+P`, or click the palette field in the title bar | The palette lists Report a problem, Reset the scale, Settings, Key help, Quit, Save, the external editor, Undo, Redo, Select all, the block, section, list and table commands and the format commands of section 10; not the caret's moves |
| 4.2 | Look at the right of each row in 4.1 | Every command the keymap binds shows its key (`Ctrl+,`, `F1`, `Ctrl+Q`, `Ctrl+S`, `Alt+↑`, `Ctrl+B`, `Ctrl+1`, `Alt+Q`); Renumber the list, Format the table, the cell moves and Turn the headings into paragraphs have none |
| 4.3 | Type `set` and press `Enter` | The settings open |
| 4.4 | Press `Esc` with the palette open | The palette closes and nothing runs |

## 5. Notices

| # | Do | Expect |
|---|---|---|
| 5.1 | Put a line weft does not know in `keymap.toml` (an action named `nothing`) and start weft | The bell shows 1 unread; opening it, a "check" notice says `nothing` is no action of weft, and the other keys still work |
| 5.2 | Break `common.toml` (an unclosed `[`) while weft is open | A notice under the bell says what could not be read; the window keeps its theme and language |
| 5.3 | Press "Mark all read" | The bell's count goes away |
| 5.4 | While weft is open, put a theme that does not exist in `common.toml`, then break `keymap.toml`, then name an event weft does not know under `[notify]` in `config.toml`; save each file again unchanged; then fix the theme and break it again | Each comes under the bell once as a "check" notice (not once a frame, not again when the same file is read again); the theme broken the second time is told again. None of them is read out by a screen reader |
| 5.5 | Make the open file read-only and press `Ctrl+S`; open the settings (or the palette), then the bell, and pick the "failed" notice | The bell and the settings (the palette) close and the document is in front with the keys; picking a notice about something else only marks it read |
| 5.6 | Make FILE.md read-only, run `weft FILE.md --keys "x<Wait:3000><C-s>"` and click another window within the 3 seconds; then click the "Could not save" system notification (or its Open button) | weft comes to the front with the document in front and the notice read. Done again with weft left in front, the failure only goes under the bell: no system notification, sound or webhook |
| 5.7 | Make FILE.md read-only, run `weft FILE.md --keys "x<Wait:3000><C-s>"` and minimize weft within the 3 seconds; then click the "Could not save" system notification (or its Open button) | weft comes back from the taskbar at its size and place, in front, with the document in front and the notice read |

## 6. Scale

| # | Do | Expect |
|---|---|---|
| 6.1 | Press `Ctrl+=` twice, then `Ctrl+-` once | The whole window grows two steps and shrinks one; the status bar says the scale |
| 6.2 | Turn the wheel with `Ctrl` held | The scale goes up and down a step at a time |
| 6.3 | Press `Ctrl+0` | The scale goes back to 100% and the other uchmk apps open at the same time follow |

## 7. Key scripts

| # | Do | Expect |
|---|---|---|
| 7.1 | Run `weft --keys <script>` with `WEFT_KEYS_DONE` set to a file, the script `<F1><Shot:help><Esc><C-,><State:settings><Quit>` | The window runs the keys without a hand on them; the file says `keys: done`, the state `settings` says `screen: settings` and `page: 0`, and a picture of the help is saved |
| 7.2 | Run a script with `<Lang:en>` before `<F1>` | The help in the picture is in English, and common.toml's language is left as it was |
| 7.3 | With a screen reader on: press `Ctrl+Z` with nothing to undo, `Ctrl+S`, `Ctrl+S` on a read-only file, `Ctrl+Q` with changes, and `→` at the end of the text | It says "Undo: nothing to do here", "Saved FILE.md", "Could not save …", and the question with its keys (Enter: Save and quit, N: Quit without saving, Esc: Cancel) once; the `→` says nothing |
| 7.4 | Run `weft FILE.md --keys "<Lang:en><C-z><Announced:Undo: nothing to do here><C-s><Announced:Saved><Quit>"` with `WEFT_KEYS_DONE` set; then the same with `<Announced:never said>` for the first check | The first run's file says `keys: done`; the second stops at that step, says `` `never said` was not announced `` and exits with 3 |
| 7.5 | In a build with `--features test-hooks`, make FILE.md read-only and run `weft FILE.md --keys "<Lang:en>x<Wait:3000><C-s><Announced:Could not save><Quit>"` with `ITO_NOTIFY_SINK` set to a file, and click another window within the 3 seconds | No system notification shows; the sink file has one `[[toast]]` titled "Could not save" whose `tag` is the notice's number, with an Open button. A build without the feature (as released) ignores `ITO_NOTIFY_SINK` |

## 8. Editing

| # | Do | Expect |
|---|---|---|
| 8.1 | Run `weft FILE.md` on a file with CRLF line ends and a BOM; move with `←` `→` over `é`, `漢字` and `👍🏽` | The caret steps over a whole character each time (the emoji with its skin tone is one); the status bar says the line and column and, by the clock, `UTF-8`, `BOM` and `CRLF` |
| 8.2 | Type a word, press `Enter`, type another; press `Ctrl+Z` three times, then `Ctrl+Y`; press `Backspace` three times quickly, then `Ctrl+Z` | The status bar says "Unsaved"; each `Ctrl+Z` takes back one step (the second word, the line break, the first word), and "Unsaved" goes once the text is back as it was read; `Ctrl+Y` puts the first word back. The three `Backspace`s come back with one `Ctrl+Z` |
| 8.3 | In a file with headings, a list and a paragraph: `Alt+↓` on the paragraph, `Alt+↓` on a list item that has items under it, `Ctrl+Enter` on a list item, `Alt+Shift+→` on a heading | The paragraph moves under the next block; the item goes under the next item of its list with the items under it (the numbers of a numbered list stay in order); the item gets `[ ]`; the heading and the headings under it go one level down. One `Ctrl+Z` puts each back whole |
| 8.4 | Press `Ctrl+S`, then look at the file in a hex viewer | "Unsaved" goes and the status bar says it saved; the file still starts with the BOM and every line ends in CRLF |
| 8.5 | Open a Shift_JIS file | The bell says it is not UTF-8 and opened read-only; the status bar says "Read-only" and, by the clock, "Not UTF-8"; typing changes nothing and `Ctrl+S` writes nothing |
| 8.6 | With changes not saved, press `Ctrl+Q` (or the close button); press `Esc`; press `Ctrl+Q` again and `N`; then, with changes again, `Ctrl+Q` and `Enter` | A question "Unsaved changes" with Cancel (Esc), Quit without saving (N) and Save and quit (Enter); `Esc` keeps the window, `N` closes it without writing, `Enter` writes the file and then closes |
| 8.7 | Open a file with headings, `**bold**`, `` `code` ``, a task list and a ` ```rust ` fence; type quickly in a long file | Headings, emphasis, code, the list markers and the Rust code have their colours; typing never waits for them (the old colours stay until the new reading comes) |
| 8.8 | Type Japanese with the IME on, then press `Enter` to commit | The text being composed shows at the caret, the candidate window sits by the caret, and the status bar says "Composing" by the clock until it is committed; the `Enter` commits and does not break the line |
| 8.9 | Open a file with CRLF line ends that holds `é` (an `e` and a combining accent), `漢字` and `👍🏽`; select part of it with `Shift+→`, then press `Ctrl+A`; open a file in Shift_JIS | The status bar counts the characters without the line breaks (`é` and `👍🏽` are one each) and, while something is selected, says "Selected" with the count of the selection (with `Ctrl+A`, the whole count); the Shift_JIS file shows no count |
| 8.10 | Type full-width spaces (`　`) between words; end a paragraph's line with two spaces and go on on the next line; end a paragraph's last line, a heading and a line in a ` ``` ` fence with two spaces too | Each full-width space has a thin, faint box where it is; the two spaces that break the paragraph's line have a faint gold tint with a small dot on each, the others none (and a `\` break none) |
| 8.11 | In a table press `Tab` and `Shift+Tab`; on a list item press `Tab` and `Shift+Tab`; in a paragraph press `Tab`, then `Shift+Tab` | In the table the caret goes to the next and the previous cell (the table is lined up); the item goes one level in and back out with the items under it; in the paragraph the tab width of spaces goes in and comes out again. One `Ctrl+Z` takes back each press; in a read-only file nothing changes (several lines: 8.24) |
| 8.12 | Open a `.md` of about 8 MB (a few lines copied over and over); type quickly; press `Ctrl+A`; press `Ctrl+Home`, then `Ctrl+Shift+End`; type over the selection, then press `Ctrl+Z` | No key hitches: each shows at once. The document's count stays the old one until the new reading comes, within about a second of the last key; "Selected" shows the selection's count, or "Selected …" until the reading comes and then the count (after `Ctrl+Z` the selection is back, and its count with it) |
| 8.13 | Look at the full-width spaces and the hard-break marks of 8.10 in a light and a dark theme; type quickly in a long file that has them | The marks are seen but quieter than the text in both themes; typing never waits for them |
| 8.14 | Turn the IME off and on, and go through its input modes | By the clock, after "Composing" and before the encoding, the status bar says "IME off", or the mode: "Hiragana", "Full-width katakana", "Half-width katakana", "Full-width alphanumeric" or "Half-width alphanumeric" ("IME on" for a mode of another language); where it cannot be told, nothing. In Japanese the words are Japanese |
| 8.15 | With Microsoft IME, press `半角/全角`, then change the input mode from its menu on the taskbar, with weft in front | The status bar follows each change at once, in the language's words; on is in the colour of the text, off is quieter |
| 8.16 | As egui sends them from Windows in one frame: type `a`, press `←`, type `b`, press `Alt+T`, type `c`; press `Ctrl+Z` three times. In an empty file: `a`, `Shift+A`, `Space`, `Enter`, `1`, `Shift+1` | `- [ ] bca`: the keys and the characters go in the order they came, and `Alt+T` types no `t`; the `Ctrl+Z`s take off `c`, then the task, then `ba`. The second gives `aA ` and `1!` on the next line, each character once |
| 8.17 | As egui sends them from Windows: in `ab` with the caret after `a`, `Ctrl+V` and `Shift+Insert` with `clip` on the clipboard; select `word` in `a word` and press `Ctrl+X`, `Shift+Delete`, `Ctrl+C`, `Ctrl+Insert`; let the key go in the same frame and in the next | Each paste puts `clip` in once; each cut takes `word` out once and puts it on the clipboard; each copy puts `word` on the clipboard and changes nothing |
| 8.18 | As egui sends them from Windows, on a Shift_JIS file with everything selected: type `a`, press `Alt+Q`, `Ctrl+B`, `Ctrl+V`, `Ctrl+X`; then `Ctrl+C` | The text stays as it was; `Ctrl+C` still copies it |
| 8.19 | On a JIS and on a US keyboard, with the IME off, type a line quickly with letters, `Shift`, `Space`, `←`, `Enter`, `Alt+T` and `Ctrl+V` among them; with the IME on, type and commit Japanese with `Alt+Q` and `Ctrl+B` between; type in a field of the settings (`Ctrl+,`) | Every character goes in once, where the caret was when it was typed; no Alt key types its letter; a paste goes in once; the committed Japanese goes in at the caret; the settings field takes what is typed and the document none of it |
| 8.20 | As egui sends them: a Korean `한` composed, then committed by `Space` (the commit, `Space` and its ` ` in one frame), then two frames more; a commit of `日本` then `a` in one frame; `a` then a commit of `日本` in one frame, then `Ctrl+Z` | `한` shows as a composition and is not typed until the commit; then the text is `한 `, and stays so (the commit goes in once); `日本a`; `a日本`, and `Ctrl+Z` takes off `日本` alone |
| 8.21 | As egui sends them from a Mac: `Option+A` (bound to nothing, it sends `å`); with two lines selected, `Option+T` (it sends `†`); then `Alt+A` from Windows | `å` goes in; `Option+T` makes the lines tasks and types no `†`; on Windows `Alt+A` types nothing |
| 8.22 | As egui sends it: select `word` and type `Alt+0165` on the numpad (the digits with `Alt` held, then `Alt` let go, then `¥`) | `¥` takes the place of `word`; nothing is struck through and no command runs |
| 8.23 | With the Korean IME, type `한` and press `Space`, then `한` and `Enter`; with the Japanese IME, type にほん, convert and press `a` without committing first; with the IME off, select a word and type `Alt+0165` on the numpad (Num Lock on) | `한 ` goes in once, the composition showing at the caret with the candidate window by it until then; after `Enter` the syllable is in once; the Japanese goes in before the `a`; `¥` takes the place of the word and nothing is struck through |
| 8.24 | Select from the middle of a paragraph line over a list (a parent item and its child) and an empty line into a ` ``` ` fence, press `Tab`, then `Shift+Tab`; select the rows of a table alone and press `Tab`; select up to the start of a line and press `Tab`; press `Ctrl+Z` | Each selected list item goes one level in and back out with the items and lines under it (the child once, with its parent); every other line gets the tab width of spaces at its start and loses them, the fence's lines too; the empty line stays empty. A table's own rows stay where they are (in a selected item they go with it); a line the selection only touches at its start stays. The selection stays on the same lines. A second `Tab` that `Shift+Tab` could not give back (the items would become code under the paragraph) does nothing; one `Ctrl+Z` takes back one press, text and selection |
| 8.25 | Turn on "Insert a tab character" (3.10); press `Tab` in a paragraph, then `Shift+Tab`; press `Shift+Tab` on a line that starts with six spaces; select three lines and press `Tab`; press `Tab` on a list item and in a table | `Tab` puts in one tab character and `Shift+Tab` takes it away; on the spaces it takes away up to the tab width; each selected line gets one tab at its start; the list item and the table cell work as in 8.11 |
| 8.26 | Select a list item, an empty line and a paragraph (`- a`, an empty line, `para`) and press `Tab`, then `Shift+Tab`; press `Tab` twice, then `Shift+Tab` twice | The item goes one level in and the paragraph by the same amount; the empty line stays empty. Each time the text comes back as it was |
| 8.27 | Select a numbered list from `1.` to `11.` and press `Tab`, then `Shift+Tab`, then twice each; in `9.`, `10.`, `11.` put the caret on `10.` alone and press `Tab`, then `Shift+Tab` | Over the selection every item goes in by one amount, so `9.` and `10.` stay in line, and the numbers do not change; it all comes back. With the caret, `10.` becomes `1.` of a list under `9.` and `11.` becomes `10.`; `Shift+Tab` gives `9.`, `10.`, `11.` back. The caret stays on its word |
| 8.28 | With the tab width 2, select from a paragraph into the code of a ` ``` ` fence that has `- x` in it and an item after it, and press `Tab`, then `Shift+Tab`; do the same in a fence inside a list item | The whole fence goes in, its closing line too; the item after it stays where it is and is still an item, not code, and `- x` stays code. `Shift+Tab` gives the text back. In a list item the fence goes with the item |
| 8.29 | Turn on "Insert a tab character" (3.10); in a CRLF file with a paragraph line, an item with a child and a line under it, select from the end to the start and press `Tab` twice, `Ctrl+Z`, `Shift+Tab`, `Ctrl+Z` | The paragraph line gets one tab; the item goes one level in as an item, in spaces, and the line under it with it; every line still ends in CRLF; the selection stays over the same lines with the caret at the start. One `Ctrl+Z` takes back one whole press, text and selection |
| 8.30 | Select several lines in a read-only file and press `Tab` and `Shift+Tab`; select only empty lines and press `Tab`; select lines and items at the left edge and press `Shift+Tab` | Nothing changes, the status bar says there is nothing to do here, and `Ctrl+Z` has nothing to take back |

## 9. The external editor and key scripts

| # | Do | Expect |
|---|---|---|
| 9.1 | Put the caret on line 3, column 5 and press `Ctrl+E` (with changes not saved) | The file is saved first, then the editor `[open]` names opens it at line 3, column 5 (at the line only, for an editor that takes no column) |
| 9.2 | Press `Ctrl+E` with an editor that does not exist in `[open]` | The bell says the external editor could not be opened |
| 9.3 | Run `weft FILE.md --keys "<C-End>x<State:typed><Quit>"` with `WEFT_KEYS_DONE` set | The state `typed` says `file`, `dirty: true`, `locked: false`, `caret` (line:column), `section` (the heading the caret is under), `lines`, `chars` (the characters, line breaks left out), `selected_chars`, `text` and `quit: no` |
| 9.4 | Run `weft +3:5 FILE.md --keys "<State:at><Quit>"` (and `weft FILE.md +3:5 …`) with `WEFT_KEYS_DONE` set | The state `at` says `caret: 3:5`; `+40` on a shorter file puts the caret on the last line |
| 9.5 | Run `weft +5000 LONG.md` on a file of more than 5000 lines | The caret's line is in view from the first frame, without scrolling, and stays in view when the Japanese font comes in |
| 9.6 | Run `weft + FILE.md`, `weft +0 FILE.md`, `weft +x FILE.md`, `weft +3: FILE.md` | Each says ``bad position `…`: give +LINE or +LINE:COL, both from 1`` and does not start |
| 9.7 | In kura, with weft as the `.md` opener, open a `.md` at a line (a heading in kura's outline; kura needs ito-openin at ceb61fd or later) | weft opens it with the caret on that line (and column, when kura passes one) |

## 10. Formatting

The keys: bold `Ctrl+B`, italic `Ctrl+I`, inline code `` Ctrl+` `` (`Ctrl+Shift+@` on JIS), a link `Ctrl+K`, headings `Ctrl+1` to `Ctrl+6`
(the same key again makes the line a paragraph), the code block `Ctrl+[` (`Ctrl+@` on JIS), the bulleted list `Ctrl+Shift+]`, the numbered list
`Ctrl+Shift+[`, strikethrough `Alt+Shift+5`, the quote `Alt+Q`, the task list `Alt+T`, Check / Uncheck the selected tasks `Alt+X` /
`Alt+Shift+X`, and Toggle the task keeps its `Ctrl+Enter`. An Alt key never types its character (but on a Mac, where Option types
characters). `Alt+5` is a key of its own and runs nothing. Turn the headings into paragraphs has no key yet: run it by name from the
palette (`Ctrl+Shift+P`).

| # | Do | Expect |
|---|---|---|
| 10.1 | Select three list items (`- a`, `- b`, `- c`) and run "Toggle bold"; run it again | Each item's text is bold (`- **a**`) with the `- ` left outside, and the same text stays selected; the second run takes the marks off |
| 10.2 | Select two plain lines from the end back to the start, run "Toggle the numbered list", press `Ctrl+Z` once, then `Ctrl+Y` | The lines become `1.` and `2.`; the one `Ctrl+Z` brings back both the text and the selection (the caret at the start); `Ctrl+Y` brings back the list and its selection |
| 10.3 | With the caret in a paragraph, run "Toggle heading 3", "Toggle heading 1", "Toggle heading 1" again; then "Toggle heading 6" and "Turn the headings into paragraphs" | `### `, then `# `, then a paragraph again, the caret staying in its word; heading 6 is taken off by the last one, which does nothing on a paragraph |
| 10.4 | Select two paragraphs with a blank line between them and run "Toggle the quote"; run it again | Every line starts with `> ` and the blank one with `>`, so it is one quote; the second run takes it off |
| 10.5 | Select two lines and run "Toggle the code block"; run it again; put the caret in a ` ```rust ` block and run it | The lines are wrapped in ` ``` ` fences with the lines still selected; the second run takes the fences off; the Rust block loses its fences and keeps its code |
| 10.6 | Select tasks, some checked, a plain item and a paragraph; run "Check the selected tasks", "Uncheck the selected tasks", then `Ctrl+Enter` twice; then `Ctrl+Enter` with the caret alone on the plain item | Check ticks every box, Uncheck clears them, the plain item and the paragraph stay as they were; with the selection `Ctrl+Enter` checks all, then unchecks all; with the caret alone it gives the plain item a box |
| 10.7 | Select two plain lines and run "Toggle the bulleted list", "Toggle the numbered list", "Toggle the task list", "Toggle the task list" | `- `, then `1.` / `2.`, then `- [ ] `, then plain lines again |
| 10.8 | In a file with CRLF line ends, run the commands of 10.1, 10.2 and 10.5 and save | The status bar still says CRLF and the saved file has CRLF on every line, the new fence lines too |
| 10.9 | Put the caret in a ` ``` ` block and run "Toggle bold", "Toggle heading 1", "Toggle the quote", "Check the selected tasks"; press `Ctrl+Z`; open a Shift_JIS file and run them | Nothing changes and `Ctrl+Z` has nothing to undo from them; the Shift_JIS file stays as it is |
| 10.10 | Select `「強調」` in `「強調」です` and run "Toggle bold"; run it again | `**「強調」**です` goes in, and the status bar says once that next to punctuation the marks may not show as emphasis (said to a screen reader too; nothing under the bell, nothing to answer); taking it off says nothing |
| 10.11 | Put the caret in a word and run "Toggle italic", "Toggle inline code" and "Toggle strikethrough" (each again to take it off); select a word and run "Toggle a link" | The word gets `*…*`, `` `…` `` and `~~…~~` with their colours; the link is `[word]()` with the caret between the parentheses, ready for the address |
| 10.12 | Press each `Ctrl` key above as egui sends it from a JIS and a US keyboard (`Ctrl+Shift+@` and `` Ctrl+` `` alike), let go in the same frame and in the next | Each runs its command once while the document has the keys, and the palette shows it by its command; none takes `F12`, the palette's, the scale's or the clipboard's key, nor a key already bound; Turn the headings into paragraphs is offered unbound on the Keys page |
| 10.13 | On a JIS keyboard, with the IME off, select a word and press `Ctrl+B`, `Ctrl+I`, `Ctrl+Shift+@` and `Ctrl+K` (each but the link twice); press `Ctrl+2` twice on a line, `Ctrl+@` over two lines, `Ctrl+Shift+]` and `Ctrl+Shift+[` over two lines; then the same on a US keyboard with `` Ctrl+` `` and `Ctrl+[`; open the key help (`F1`) and the palette | Each key does what its command does in 10.1–10.11 (the second `Ctrl+2` makes the line a paragraph again) and types nothing; the key help and the palette list each key by its command |
| 10.14 | Press each `Alt` key above as egui sends it from Windows (the key, then its character: `Alt+Shift+5` sends `%`); press `Alt+Z` and, with a word selected, `Alt+5`, which run nothing | Each runs its command as the palette runs it by name and types nothing; the palette and the key help show `Alt+Q`, `Alt+T`, `Alt+Shift+5`, `Alt+X`, `Alt+Shift+X` by their commands; none is `F12`, the palette's, the scale's or the clipboard's key, and none is bound twice; `Alt+Z` and `Alt+5` change nothing and type nothing |
| 10.15 | On a JIS and on a US keyboard, with the IME off and then on, select two lines and press `Alt+Q`, `Alt+T` (each twice); select a word and press `Alt+Shift+5` (twice), then `Alt+5`; select tasks and press `Alt+X`, then `Alt+Shift+X`; open the key help (`F1`) and the palette | Each does what its command does in 10.4, 10.6, 10.7 and 10.11 and types nothing, with the IME on too; `Alt+5` does nothing; the window's menu does not open; the key help and the palette list each key by its command |

## 11. Structure commands

The keys: Expand the selection `Alt+W`, Shrink the selection `Alt+Shift+W`, To the next heading `Alt+N`, To the previous heading `Alt+P`,
To the parent heading `Alt+U`. They type no letter. The rest (the table of contents, the deletes, the other moves) have no key: run them by
name from the palette (`Ctrl+Shift+P`), or bind one on the Keys page of the settings.

| # | Do | Expect |
|---|---|---|
| 11.1 | Put the caret in the paragraph under the first heading and run "Insert or update the table of contents"; press `Ctrl+Z`, then `Ctrl+Y`; add a heading at the end, put the caret at the top and run it again, then once more | A `<!-- toc -->` … `<!-- /toc -->` list of the headings goes in after the paragraph, the caret on its first line; one `Ctrl+Z` takes it all out; run again, the same list takes in the new heading (there is still one list), and the last run does nothing |
| 11.2 | Put the caret in a word of a sentence and run "Expand the selection" until it stops; then "Shrink the selection" until it stops; do the same from a few letters selected backwards | The selection grows to the word, the sentence, the paragraph, the section and the whole text, the caret at its end; Shrink goes back the same steps to the caret where it was; the backwards one grows backwards and comes back to the same letters; `Ctrl+Z` has nothing to undo from them |
| 11.3 | Expand to the section, type a character somewhere (or type and undo it), select the section again and run "Shrink the selection"; expand to the section again, press `Shift+→` and shrink twice | After an edit, Shrink takes the largest thing inside the selection that starts where it starts (the heading line), not the paragraph it grew from; after the move it gives the section, then the heading line |
| 11.4 | Put the caret in the body under a `##` heading that has a `###` under it and run "Delete the heading line, keeping its body", "Delete the section with its subsections", "Turn this section's heading into a paragraph" and "Delete the heading and lift its subheadings", each followed by `Ctrl+Z`; then run them above the first heading | The line alone goes; the section goes with its `###`; the heading becomes a plain paragraph; the heading goes and the `###` becomes `##`; one `Ctrl+Z` brings back each; above the first heading none of them does anything |
| 11.5 | In a document with headings, lists, a table, a code block and open tasks, run "To the next heading" / "To the previous heading", and the same for lists, tables, code blocks and open tasks; run "To the parent heading" and "To the first subheading" | The caret goes to the start of the next or previous one of its kind (an open task is `- [ ]`, not `- [x]`), the selection dropped and the view following it; the parent goes to the heading of the section (on a heading line, to the one above it), the child to the first heading inside the section; the text does not change and `Ctrl+Z` has nothing to undo |
| 11.6 | Run each of 11.1–11.5 where it has nothing to do (no heading above, the last heading, a bare caret for Shrink), in English and in Japanese | Nothing changes and a screen reader says "…: nothing to do here" / 「…: ここでは何もできません」 with the command's name |
| 11.7 | Open a CRLF file with a BOM, run "Insert or update the table of contents" and "Turn this section's heading into a paragraph" on a setext heading, and save | The status bar still says CRLF and BOM; the saved file has CRLF on every line, the list's lines too, and starts with the BOM |
| 11.8 | Open a Shift_JIS file and run 11.1, 11.4, 11.5 and 11.2 | The table of contents and the deletes change nothing; the moves, Expand and Shrink the selection still move the caret and the selection; the file is not marked unsaved |
| 11.9 | Open the palette in Japanese and type 目次, 選択, 見出し; bind "Expand the selection" to a key on the Keys page and open the key help (`F1`); select `Wow!́` (the `!` with a combining accent) by Expand from the `W` | Every new command is listed by its Japanese name; the key help lists the bound key by the command's name; the second Expand takes the `!` with its accent, and Shrink never leaves the accent apart from it |
| 11.10 | Press `Alt+W`, then `Alt+Shift+W` after two `Alt+W`s, `Alt+N`, `Alt+P` and `Alt+U` as egui sends them from Windows (the key, then its letter), with the caret in a section's body | Each selects or moves as the palette runs its command by name, and the text does not change (no letter typed); the palette and the key help show `Alt+W`, `Alt+Shift+W`, `Alt+N`, `Alt+P`, `Alt+U` by their commands; none is `F12`, the palette's, the scale's or the clipboard's key, and none is bound twice |
| 11.11 | On a JIS and on a US keyboard, with the IME off and then on, do 11.2 with `Alt+W` and `Alt+Shift+W`, and 11.5's headings with `Alt+N`, `Alt+P` and `Alt+U`; open the key help (`F1`) and the palette | Each does what its command does in 11.2 and 11.5 and types no letter, with the IME on too; the window's menu does not open; the key help and the palette list each key by its command |
| 11.12 | In `# P`, `body`, `> ## Q`, `q` (blank lines between), put the caret at the start of the `> ## Q` line and run "Delete the heading line, keeping its body"; press `Ctrl+Z`; the same with `- ## L` and ` ## C` (one space) for the line | The line goes with its `> ` (`- `, ` `) and the blank line after it: `# P`, `body`, `q` are left, `# P` untouched; one `Ctrl+Z` brings it back |

## 12. Tabs

| # | Do | Expect |
|---|---|---|
| 12.1 | Start `weft a.md`, then run `weft b.md` again from another terminal | The running window comes forward with b.md as a second tab; no second window opens |
| 12.2 | Press `Ctrl+Tab`, `Ctrl+Shift+Tab`, `Alt+2`, and `Ctrl+W` on a tab with unsaved changes | The tab changes each time; `Ctrl+W` asks to save, discard or cancel before closing |

## 13. A second launch

| # | Do | Expect |
|---|---|---|
| 13.1 | Run `weft sub\c.md` from a folder while weft is already running | The request is the file made absolute against where it was run, so c.md opens by its full path |
| 13.2 | Run `weft b.md` while weft is already running | b.md arrives as a tab beside the front one |
| 13.3 | Run `weft b.md` while weft is already running | The request reaches the running weft through its door |
| 13.4 | Run `weft a.md` while a.md is already open | The tab for a.md comes forward; no duplicate tab |
| 13.5 | Quit weft and run it again | The first weft owns the door again; a stale door file does not stop it |

## 14. Find and replace

The keys: Find `Ctrl+F`, Find and replace `Ctrl+H`, Next match `F3`, Previous match `Shift+F3`, Replace all `Ctrl+Alt+Enter`.
In the bar `Enter` goes to the next match, `Shift+Enter` to the previous, `Enter` in the Replace field replaces, `Esc` closes the bar.

| # | Do | Expect |
|---|---|---|
| 14.1 | Replace all with a regex and `$1` in the replacement | Every place is replaced; one undo brings all of them back |
| 14.2 | Select two lines, turn on "In selection", search for a word that is also outside them, and replace all | Only the selected lines change |
| 14.3 | Open a Shift_JIS file, press `Ctrl+H` and replace all | Nothing changes; the bar says the file cannot be changed |
| 14.4 | Press `F3` past the last match, `Shift+F3` before the first | The step wraps round to the other end |
| 14.5 | Replace the match the selection is on | The replacement goes in and the next match is selected |
| 14.6 | An old result arrives after a newer query | It is not the one shown |
| 14.7 | Turn on "Regular expression" and type `(` | The bar says why it is not a pattern; replace does nothing |
| 14.8 | Press `Ctrl+F`, type a word | Every match is tinted in the text and the one at the caret is tinted stronger; the bar shows "i / n", or "No results" |
| 14.9 | Press `Ctrl+F` with a word selected; press `Esc` | The word is in the field; `Esc` closes the bar and the highlights go |
| 14.10 | With the bar focused type text; click into the document and type | While the bar has the focus nothing goes into the document; after the click it does |
| 14.11 | Turn on the Japanese IME and type `かな` into the field, press `Enter` while converting, then `Enter` again | The first `Enter` only confirms the conversion; the second steps to the next match |
| 14.12 | Open two files as tabs, search in one, switch to the other and back | Each tab shows its own count and highlights; the bar's query stays |
| 14.13 | Replace all in a CRLF file with a BOM, save, and look at the file | The line endings and the BOM are as they were |
| 14.14 | Look at the bar in the light and the dark theme, in a narrow window and with a long query | The bar uses the theme's colours, nothing overlaps, the buttons are the shared ones with the focus ring |
| 14.15 | In a document where a word occurs several times, press `Ctrl+H`, type the word and a replacement, press `Ctrl+Alt+Enter`, then `Ctrl+Z` once | Every place is replaced and the status bar says how many; one `Ctrl+Z` brings all of them back |
| 14.16 | With a match selected, press `Enter` in the Replace field repeatedly | Each press replaces that match and moves to the next one |
| 14.17 | Type a long query one letter at a time in a large file | Typing never waits; the count and the highlights follow the newest query |

## 15. Rendered preview

Toggle `Ctrl+Shift+M` (the palette: "Toggle the preview"). The pane is on the right, resizable, and is closed in a window narrower than 720 points.

| # | Do | Expect |
|---|---|---|
| 15.1 | Edit the text while the preview is open | The preview is laid out for the newest text and width; an older result is not shown |
| 15.2 | Scroll the source, then scroll the preview | The other pane follows; neither jumps back |
| 15.3 | Narrow the window below 720 points, or close every file | The preview is not shown |
| 15.4 | Press `Ctrl+Shift+M` twice, quit and run weft again | It toggles; the last state comes back |
| 15.5 | Click a line of the preview | The caret goes to that place in the source (never inside a character) |
| 15.6 | Press `Ctrl+F` and type a word | The matches are tinted in the preview too |
| 15.7 | Open an empty file with the preview on | The pane says there is nothing to preview |
| 15.8 | Type in a large file with the preview open | Typing never waits; the preview catches up |
| 15.9 | Look at the pane in the light and the dark theme, with long Japanese lines, a wide table and a code block, and while resizing | Lines wrap at the pane's width; colours are the theme's; nothing overlaps |

## 16. Minimap

| # | Do | Expect |
|---|---|---|
| 16.1 | Open a document of a few hundred lines in a wide window | A strip at the right edge shows each line as a band in its colour; the frame marks what is on screen |
| 16.2 | Click and drag in the strip | The source scrolls there; no caret is put by the click; hovering shows the lines around the pointer |
| 16.3 | Narrow the window, or open the preview until the source is narrow | The strip goes away and the text takes the room |

## 17. Contents column

| # | Do | Expect |
|---|---|---|
| 17.1 | Open a document with several headings, open the preview in a wide window (Ctrl+Shift+M), in the light and dark themes | A "Contents" column at the right edge of the preview lists the headings, indented by level; the one being read is marked |
| 17.2 | Click an entry | The preview scrolls to that heading and the caret goes to its start; the source follows |
| 17.3 | Narrow the preview (drag its edge), or open a document with no headings | The column goes away |

## 18. Word wrap

| # | Do | Expect |
|---|---|---|
| 18.1 | Open a document with long lines in a narrow window and press Alt+Z | Long lines fold at the pane's width; the status line announces it; line numbers sit at each line's first row |
| 18.2 | With wrap on, type, select across folded lines, search, and click in a lower row | The caret, selection and match bands follow each row; the click puts the caret in that row |
| 18.3 | Press Alt+Z again, then restart the app | Lines scroll sideways again; the last choice is kept across restarts |
