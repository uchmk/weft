# Checklist

55 checks in TESTING.md, 43 left for a person.

## 1. Start

自動テスト済みなので下には出していない: 1.8

- [ ] **1.1** Run `weft` → A window of 1280x800 opens with the title "weft" 〔未訳〕
- [ ] **1.2** Move and resize the window, close it and start it again → The window comes back at the same place and size 〔未訳〕
- [ ] **1.3** Press `F12` → The browser opens the repository's new issue form with the version and OS filled in 〔未訳〕
- [ ] **1.4** Look at the title bar → There is no system title bar; at the left the woven icon (three teal threads across, three grey threads up and down, going over and under) and "weft"; the palette field, the bell and the minimize, maximize and close buttons at the right 〔未訳〕
- [ ] **1.5** Run `weft` with no file → An empty-state ghost in the middle ("No file is open" over "Start weft with a Markdown file: weft FILE.md"), with no button under it 〔未訳〕
- [ ] **1.6** Look at `weft.exe` in Explorer and on the taskbar → The same woven icon as in the title bar 〔未訳〕
- [ ] **1.7** Press `Ctrl+Q` with nothing unsaved → The window closes; started again, it comes back at the place it closed at 〔未訳〕

## 2. Key help

自動テスト済みなので下には出していない: 2.5

- [ ] **2.1** Press `F1` → A card over the dimmed window lists the keys in groups: "Everywhere" (settings, key help, quit, save, the external editor, undo and redo), "Editing" (the caret, the selection, typing and the structure commands), "Always" (F12, the palette, the scale) and "This list" 〔未訳〕
- [ ] **2.2** Press `↓`, `PgDn`, `End`, `Home`, and turn the wheel over the card (in a window small enough that the list scrolls) → The list scrolls a line, a page, to the end and back to the top; the wheel scrolls by lines 〔未訳〕
- [ ] **2.3** Press `Esc`; open the help again and press `F1`; open it again and click outside the card → Each time the help closes 〔未訳〕
- [ ] **2.4** Type `?` → Nothing opens: `?` is left for the text, only `F1` opens the help 〔未訳〕

## 3. Settings

自動テスト済みなので下には出していない: 3.4

- [ ] **3.1** Press `Ctrl+,` → The settings open on the General page (language, size, clock) 〔未訳〕
- [ ] **3.2** Press `Ctrl+Tab` until the pages come round → General, Theme, Keys, Notifications, Advanced and About, in that order 〔未訳〕
- [ ] **3.3** On the Keys page, change the key of Quit to `Ctrl+W` → The row shows `Ctrl+W`; `Ctrl+W` in the window quits, `Ctrl+Q` no longer does 〔未訳〕
- [ ] **3.5** On the Keys page, give a binding `F12` → The page says weft reads `F12` itself, so the binding never runs 〔未訳〕
- [ ] **3.6** On the Notifications page, press the test button of "Could not save" with the window behind another one → A system notification "weft: test notification" arrives (and the sound, if one is chosen) 〔未訳〕
- [ ] **3.7** Change the theme on the Theme page → The window and the sample change colour at once; another uchmk app open at the same time follows within 2 seconds 〔未訳〕
- [ ] **3.8** Press `Esc` → The settings close and the window comes back 〔未訳〕

## 4. Command palette

自動テスト済みなので下には出していない: 4.2

- [ ] **4.1** Press `Ctrl+Shift+P`, or click the palette field in the title bar → The palette lists Report a problem, Reset the scale, Settings, Key help, Quit, Save, the external editor, Undo, Redo, Select all and the block, section, list and table commands; not the caret's moves 〔未訳〕
- [ ] **4.3** Type `set` and press `Enter` → The settings open 〔未訳〕
- [ ] **4.4** Press `Esc` with the palette open → The palette closes and nothing runs 〔未訳〕

## 5. Notices

自動テスト済みなので下には出していない: 5.4, 5.5

- [ ] **5.1** Put a line weft does not know in `keymap.toml` (an action named `nothing`) and start weft → The bell shows 1 unread; opening it, a "check" notice says `nothing` is no action of weft, and the other keys still work 〔未訳〕
- [ ] **5.2** Break `common.toml` (an unclosed `[`) while weft is open → A notice under the bell says what could not be read; the window keeps its theme and language 〔未訳〕
- [ ] **5.3** Press "Mark all read" → The bell's count goes away 〔未訳〕
- [ ] **5.6** Make FILE.md read-only, run `weft FILE.md --keys "x<Wait:3000><C-s>"` and click another window within the 3 seconds; then click the "Could not save" system notification (or its Open button) → weft comes to the front with the document in front and the notice read. Done again with weft left in front, the failure only goes under the bell: no system notification, sound or webhook 〔未訳〕

## 6. Scale

- [ ] **6.1** Press `Ctrl+=` twice, then `Ctrl+-` once → The whole window grows two steps and shrinks one; the status bar says the scale 〔未訳〕
- [ ] **6.2** Turn the wheel with `Ctrl` held → The scale goes up and down a step at a time 〔未訳〕
- [ ] **6.3** Press `Ctrl+0` → The scale goes back to 100% and the other uchmk apps open at the same time follow 〔未訳〕

## 7. Key scripts

自動テスト済みなので下には出していない: 7.3

- [ ] **7.1** Run `weft --keys <script>` with `WEFT_KEYS_DONE` set to a file, the script `<F1><Shot:help><Esc><C-,><State:settings><Quit>` → The window runs the keys without a hand on them; the file says `keys: done`, the state `settings` says `screen: settings` and `page: 0`, and a picture of the help is saved 〔未訳〕
- [ ] **7.2** Run a script with `<Lang:en>` before `<F1>` → The help in the picture is in English, and common.toml's language is left as it was 〔未訳〕
- [ ] **7.4** Run `weft FILE.md --keys "<Lang:en><C-z><Announced:Undo: nothing to do here><C-s><Announced:Saved><Quit>"` with `WEFT_KEYS_DONE` set; then the same with `<Announced:never said>` for the first check → The first run's file says `keys: done`; the second stops at that step, says `` `never said` was not announced `` and exits with 3 〔未訳〕
- [ ] **7.5** In a build with `--features test-hooks`, make FILE.md read-only and run `weft FILE.md --keys "<Lang:en>x<Wait:3000><C-s><Announced:Could not save><Quit>"` with `ITO_NOTIFY_SINK` set to a file, and click another window within the 3 seconds → No system notification shows; the sink file has one `[[toast]]` titled "Could not save" whose `tag` is the notice's number, with an Open button. A build without the feature (as released) ignores `ITO_NOTIFY_SINK` 〔未訳〕

## 8. Editing

自動テスト済みなので下には出していない: 8.6, 8.10

- [ ] **8.1** Run `weft FILE.md` on a file with CRLF line ends and a BOM; move with `←` `→` over `é`, `漢字` and `👍🏽` → The caret steps over a whole character each time (the emoji with its skin tone is one); the status bar says the line and column, `CRLF` and `BOM` 〔未訳〕
- [ ] **8.2** Type a word, press `Enter`, type another; press `Ctrl+Z` three times, then `Ctrl+Y`; press `Backspace` three times quickly, then `Ctrl+Z` → The status bar says "Unsaved"; each `Ctrl+Z` takes back one step (the second word, the line break, the first word), and "Unsaved" goes once the text is back as it was read; `Ctrl+Y` puts the first word back. The three `Backspace`s come back with one `Ctrl+Z` 〔未訳〕
- [ ] **8.3** In a file with headings, a list and a paragraph: `Alt+↓` on the paragraph, `Alt+↓` on a list item that has items under it, `Ctrl+Enter` on a list item, `Alt+Shift+→` on a heading → The paragraph moves under the next block; the item goes under the next item of its list with the items under it (the numbers of a numbered list stay in order); the item gets `[ ]`; the heading and the headings under it go one level down. One `Ctrl+Z` puts each back whole 〔未訳〕
- [ ] **8.4** Press `Ctrl+S`, then look at the file in a hex viewer → "Unsaved" goes and the status bar says it saved; the file still starts with the BOM and every line ends in CRLF 〔未訳〕
- [ ] **8.5** Open a Shift_JIS file → The bell says it is not UTF-8 and opened read-only; the status bar says "Read-only"; typing changes nothing and `Ctrl+S` writes nothing 〔未訳〕
- [ ] **8.7** Open a file with headings, `**bold**`, `` `code` ``, a task list and a ` ```rust ` fence; type quickly in a long file → Headings, emphasis, code, the list markers and the Rust code have their colours; typing never waits for them (the old colours stay until the new reading comes) 〔未訳〕
- [ ] **8.8** Type Japanese with the IME on, then press `Enter` to commit → The text being composed shows at the caret, the candidate window sits by the caret; the `Enter` commits and does not break the line 〔未訳〕

## 9. The external editor and key scripts

自動テスト済みなので下には出していない: 9.4, 9.5, 9.6

- [ ] **9.1** Put the caret on line 3, column 5 and press `Ctrl+E` (with changes not saved) → The file is saved first, then the editor `[open]` names opens it at line 3, column 5 (at the line only, for an editor that takes no column) 〔未訳〕
- [ ] **9.2** Press `Ctrl+E` with an editor that does not exist in `[open]` → The bell says the external editor could not be opened 〔未訳〕
- [ ] **9.3** Run `weft FILE.md --keys "<C-End>x<State:typed><Quit>"` with `WEFT_KEYS_DONE` set → The state `typed` says `file`, `dirty: true`, `locked: false`, `caret` (line:column), `section` (the heading the caret is under), `lines`, `text` and `quit: no` 〔未訳〕
- [ ] **9.7** In kura, with weft as the `.md` opener, open a `.md` at a line (a heading in kura's outline; kura needs ito-openin at ceb61fd or later) → weft opens it with the caret on that line (and column, when kura passes one) 〔未訳〕
