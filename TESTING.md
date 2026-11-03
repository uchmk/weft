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
| 3.2 | Press `Ctrl+Tab` until the pages come round | General, Theme, Keys, Notifications, Advanced and About, in that order |
| 3.3 | On the Keys page, change the key of Quit to `Ctrl+W` | The row shows `Ctrl+W`; `Ctrl+W` in the window quits, `Ctrl+Q` no longer does |
| 3.4 | Open `keymap.toml` in weft's config folder after 3.3, then press ↺ on the row | Only the changed binding is written; after ↺ nothing of it is left |
| 3.5 | On the Keys page, give a binding `F12` | The page says weft reads `F12` itself, so the binding never runs |
| 3.6 | On the Notifications page, press the test button of "Could not save" with the window behind another one | A system notification "weft: test notification" arrives (and the sound, if one is chosen) |
| 3.7 | Change the theme on the Theme page | The window and the sample change colour at once; another uchmk app open at the same time follows within 2 seconds |
| 3.8 | Press `Esc` | The settings close and the window comes back |

## 4. Command palette

| # | Do | Expect |
|---|---|---|
| 4.1 | Press `Ctrl+Shift+P`, or click the palette field in the title bar | The palette lists Report a problem, Reset the scale, Settings, Key help, Quit, Save, the external editor, Undo, Redo, Select all and the block, section, list and table commands; not the caret's moves |
| 4.2 | Look at the right of each row in 4.1 | Every command the keymap binds shows its key (`Ctrl+,`, `F1`, `Ctrl+Q`, `Ctrl+S`, `Alt+↑`); Renumber the list, Format the table and the cell moves have none |
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
| 8.12 | Open a `.md` of about 8 MB (a few lines copied over and over); type quickly; press `Ctrl+A`; press `Ctrl+Home`, then `Ctrl+Shift+End`; type over the selection, then press `Ctrl+Z` | No key hitches: each shows at once. The document's count stays the old one until the new reading comes, within about a second of the last key; "Selected" shows the selection's count, or "Selected …" until the reading comes and then the count (after `Ctrl+Z` the selection is back, and its count with it) |
| 8.13 | Look at the full-width spaces and the hard-break marks of 8.10 in a light and a dark theme; type quickly in a long file that has them | The marks are seen but quieter than the text in both themes; typing never waits for them |
| 8.14 | Turn the IME off and on, and go through its input modes | By the clock, after "Composing" and before the encoding, the status bar says "IME off", or the mode: "Hiragana", "Full-width katakana", "Half-width katakana", "Full-width alphanumeric" or "Half-width alphanumeric" ("IME on" for a mode of another language); where it cannot be told, nothing. In Japanese the words are Japanese |
| 8.15 | With Microsoft IME, press `半角/全角`, then change the input mode from its menu on the taskbar, with weft in front | The status bar follows each change at once, in the language's words; on is in the colour of the text, off is quieter |

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
