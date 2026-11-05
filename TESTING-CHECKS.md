# Checklist

194 checks in TESTING.md, 127 left for a person.

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
- [ ] **3.2** Press `Ctrl+Tab` until the pages come round → General, Theme, Editing, Keys, Notifications, Advanced and About, in that order 〔未訳〕
- [ ] **3.3** On the Keys page, change the key of Quit to `Ctrl+W` → The row shows `Ctrl+W`; `Ctrl+W` in the window quits, `Ctrl+Q` no longer does 〔未訳〕
- [ ] **3.5** On the Keys page, give a binding `F12` → The page says weft reads `F12` itself, so the binding never runs 〔未訳〕
- [ ] **3.6** On the Notifications page, press the test button of "Could not save" with the window behind another one → A system notification "weft: test notification" arrives (and the sound, if one is chosen) 〔未訳〕
- [ ] **3.7** Change the theme on the Theme page → The window and the sample change colour at once; another uchmk app open at the same time follows within 2 seconds 〔未訳〕
- [ ] **3.8** Press `Esc` → The settings close and the window comes back 〔未訳〕
- [ ] **3.9** On the Editing page, pick 2 as the tab width, close the settings and press `Tab` in a paragraph, then `Tab` with the IME composing; open `config.toml` in weft's config folder → The page offers 2, 4 and 8 (4 at first); `Tab` puts in 2 spaces at once and the focus stays in the text (no button of the bar lights up); while composing the IME keeps the key; the file has `tab_width = 2` under `[editor]` and its other lines as they were 〔未訳〕
- [ ] **3.10** On the Editing page, turn on "Insert a tab character" (with the keys too), close the settings and press `Tab` in a paragraph; open `config.toml` in weft's config folder; turn it off again → The switch sits under the tab width, with its note; `Tab` puts in one tab character; the file has `tab_char = true` under `[editor]` beside `tab_width`, and its other lines as they were; turned off, it says `tab_char = false` and `Tab` puts in spaces again 〔未訳〕

## 4. Command palette

自動テスト済みなので下には出していない: 4.2

- [ ] **4.1** Press `Ctrl+Shift+P`, or click the palette field in the title bar → The palette lists Report a problem, Reset the scale, Settings, Key help, Quit, Save, the external editor, Undo, Redo, Select all, the block, section, list and table commands and the format commands of section 10; not the caret's moves 〔未訳〕
- [ ] **4.3** Type `set` and press `Enter` → The settings open 〔未訳〕
- [ ] **4.4** Press `Esc` with the palette open → The palette closes and nothing runs 〔未訳〕

## 5. Notices

自動テスト済みなので下には出していない: 5.4, 5.5

- [ ] **5.1** Put a line weft does not know in `keymap.toml` (an action named `nothing`) and start weft → The bell shows 1 unread; opening it, a "check" notice says `nothing` is no action of weft, and the other keys still work 〔未訳〕
- [ ] **5.2** Break `common.toml` (an unclosed `[`) while weft is open → A notice under the bell says what could not be read; the window keeps its theme and language 〔未訳〕
- [ ] **5.3** Press "Mark all read" → The bell's count goes away 〔未訳〕
- [ ] **5.6** Make FILE.md read-only, run `weft FILE.md --keys "x<Wait:3000><C-s>"` and click another window within the 3 seconds; then click the "Could not save" system notification (or its Open button) → weft comes to the front with the document in front and the notice read. Done again with weft left in front, the failure only goes under the bell: no system notification, sound or webhook 〔未訳〕
- [ ] **5.7** Make FILE.md read-only, run `weft FILE.md --keys "x<Wait:3000><C-s>"` and minimize weft within the 3 seconds; then click the "Could not save" system notification (or its Open button) → weft comes back from the taskbar at its size and place, in front, with the document in front and the notice read 〔未訳〕

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

自動テスト済みなので下には出していない: 8.6, 8.9, 8.10, 8.11, 8.14, 8.16, 8.17, 8.18, 8.20, 8.21, 8.22, 8.24, 8.25, 8.26, 8.27, 8.28, 8.29, 8.30

- [ ] **8.1** Run `weft FILE.md` on a file with CRLF line ends and a BOM; move with `←` `→` over `é`, `漢字` and `👍🏽` → The caret steps over a whole character each time (the emoji with its skin tone is one); the status bar says the line and column and, by the clock, `UTF-8`, `BOM` and `CRLF` 〔未訳〕
- [ ] **8.2** Type a word, press `Enter`, type another; press `Ctrl+Z` three times, then `Ctrl+Y`; press `Backspace` three times quickly, then `Ctrl+Z` → The status bar says "Unsaved"; each `Ctrl+Z` takes back one step (the second word, the line break, the first word), and "Unsaved" goes once the text is back as it was read; `Ctrl+Y` puts the first word back. The three `Backspace`s come back with one `Ctrl+Z` 〔未訳〕
- [ ] **8.3** In a file with headings, a list and a paragraph: `Alt+↓` on the paragraph, `Alt+↓` on a list item that has items under it, `Ctrl+Enter` on a list item, `Alt+Shift+→` on a heading → The paragraph moves under the next block; the item goes under the next item of its list with the items under it (the numbers of a numbered list stay in order); the item gets `[ ]`; the heading and the headings under it go one level down. One `Ctrl+Z` puts each back whole 〔未訳〕
- [ ] **8.4** Press `Ctrl+S`, then look at the file in a hex viewer → "Unsaved" goes and the status bar says it saved; the file still starts with the BOM and every line ends in CRLF 〔未訳〕
- [ ] **8.5** Open a Shift_JIS file → The bell says it is not UTF-8 and opened read-only; the status bar says "Read-only" and, by the clock, "Not UTF-8"; typing changes nothing and `Ctrl+S` writes nothing 〔未訳〕
- [ ] **8.7** Open a file with headings, `**bold**`, `` `code` ``, a task list and a ` ```rust ` fence; type quickly in a long file → Headings, emphasis, code, the list markers and the Rust code have their colours; typing never waits for them (the old colours stay until the new reading comes) 〔未訳〕
- [ ] **8.8** Type Japanese with the IME on, then press `Enter` to commit → The text being composed shows at the caret, the candidate window sits by the caret, and the status bar says "Composing" by the clock until it is committed; the `Enter` commits and does not break the line 〔未訳〕
- [ ] **8.12** Open a `.md` of about 8 MB (a few lines copied over and over); type quickly; press `Ctrl+A`; press `Ctrl+Home`, then `Ctrl+Shift+End`; type over the selection, then press `Ctrl+Z` → No key hitches: each shows at once. The document's count stays the old one until the new reading comes, within about a second of the last key; "Selected" shows the selection's count, or "Selected …" until the reading comes and then the count (after `Ctrl+Z` the selection is back, and its count with it) 〔未訳〕
- [ ] **8.13** Look at the full-width spaces and the hard-break marks of 8.10 in a light and a dark theme; type quickly in a long file that has them → The marks are seen but quieter than the text in both themes; typing never waits for them 〔未訳〕
- [ ] **8.15** With Microsoft IME, press `半角/全角`, then change the input mode from its menu on the taskbar, with weft in front → The status bar follows each change at once, in the language's words; on is in the colour of the text, off is quieter 〔未訳〕
- [ ] **8.19** On a JIS and on a US keyboard, with the IME off, type a line quickly with letters, `Shift`, `Space`, `←`, `Enter`, `Alt+T` and `Ctrl+V` among them; with the IME on, type and commit Japanese with `Alt+Q` and `Ctrl+B` between; type in a field of the settings (`Ctrl+,`) → Every character goes in once, where the caret was when it was typed; no Alt key types its letter; a paste goes in once; the committed Japanese goes in at the caret; the settings field takes what is typed and the document none of it 〔未訳〕
- [ ] **8.23** With the Korean IME, type `한` and press `Space`, then `한` and `Enter`; with the Japanese IME, type にほん, convert and press `a` without committing first; with the IME off, select a word and type `Alt+0165` on the numpad (Num Lock on) → `한 ` goes in once, the composition showing at the caret with the candidate window by it until then; after `Enter` the syllable is in once; the Japanese goes in before the `a`; `¥` takes the place of the word and nothing is struck through 〔未訳〕

## 9. The external editor and key scripts

自動テスト済みなので下には出していない: 9.4, 9.5, 9.6

- [ ] **9.1** Put the caret on line 3, column 5 and press `Ctrl+E` (with changes not saved) → The file is saved first, then the editor `[open]` names opens it at line 3, column 5 (at the line only, for an editor that takes no column) 〔未訳〕
- [ ] **9.2** Press `Ctrl+E` with an editor that does not exist in `[open]` → The bell says the external editor could not be opened 〔未訳〕
- [ ] **9.3** Run `weft FILE.md --keys "<C-End>x<State:typed><Quit>"` with `WEFT_KEYS_DONE` set → The state `typed` says `file`, `dirty: true`, `locked: false`, `caret` (line:column), `section` (the heading the caret is under), `lines`, `chars` (the characters, line breaks left out), `selected_chars`, `text` and `quit: no` 〔未訳〕
- [ ] **9.7** In kura, with weft as the `.md` opener, open a `.md` at a line (a heading in kura's outline; kura needs ito-openin at ceb61fd or later) → weft opens it with the caret on that line (and column, when kura passes one) 〔未訳〕

## 10. Formatting

自動テスト済みなので下には出していない: 10.1, 10.2, 10.3, 10.4, 10.5, 10.6, 10.7, 10.8, 10.9, 10.10, 10.12, 10.14

- [ ] **10.11** Put the caret in a word and run "Toggle italic", "Toggle inline code" and "Toggle strikethrough" (each again to take it off); select a word and run "Toggle a link" → The word gets `*…*`, `` `…` `` and `~~…~~` with their colours; the link is `[word]()` with the caret between the parentheses, ready for the address 〔未訳〕
- [ ] **10.13** On a JIS keyboard, with the IME off, select a word and press `Ctrl+B`, `Ctrl+I`, `Ctrl+Shift+@` and `Ctrl+K` (each but the link twice); press `Ctrl+2` twice on a line, `Ctrl+@` over two lines, `Ctrl+Shift+]` and `Ctrl+Shift+[` over two lines; then the same on a US keyboard with `` Ctrl+` `` and `Ctrl+[`; open the key help (`F1`) and the palette → Each key does what its command does in 10.1–10.11 (the second `Ctrl+2` makes the line a paragraph again) and types nothing; the key help and the palette list each key by its command 〔未訳〕
- [ ] **10.15** On a JIS and on a US keyboard, with the IME off and then on, select two lines and press `Alt+Q`, `Alt+T` (each twice); select a word and press `Alt+Shift+5` (twice), then `Alt+5`; select tasks and press `Alt+X`, then `Alt+Shift+X`; open the key help (`F1`) and the palette → Each does what its command does in 10.4, 10.6, 10.7 and 10.11 and types nothing, with the IME on too; `Alt+5` does nothing; the window's menu does not open; the key help and the palette list each key by its command 〔未訳〕

## 11. Structure commands

自動テスト済みなので下には出していない: 11.1, 11.2, 11.3, 11.4, 11.5, 11.6, 11.7, 11.8, 11.9, 11.10, 11.12

- [ ] **11.11** On a JIS and on a US keyboard, with the IME off and then on, do 11.2 with `Alt+W` and `Alt+Shift+W`, and 11.5's headings with `Alt+N`, `Alt+P` and `Alt+U`; open the key help (`F1`) and the palette → Each does what its command does in 11.2 and 11.5 and types no letter, with the IME on too; the window's menu does not open; the key help and the palette list each key by its command 〔未訳〕

## 12. Tabs

- [ ] **12.1** Start `weft a.md`, then run `weft b.md` again from another terminal → The running window comes forward with b.md as a second tab; no second window opens 〔未訳〕
- [ ] **12.2** Press `Ctrl+Tab`, `Ctrl+Shift+Tab`, `Alt+2`, and `Ctrl+W` on a tab with unsaved changes → The tab changes each time; `Ctrl+W` asks to save, discard or cancel before closing 〔未訳〕

## 13. A second launch

自動テスト済みなので下には出していない: 13.1, 13.2, 13.3

- [ ] **13.4** Run `weft a.md` while a.md is already open → The tab for a.md comes forward; no duplicate tab 〔未訳〕
- [ ] **13.5** Quit weft and run it again → The first weft owns the door again; a stale door file does not stop it 〔未訳〕

## 14. Find and replace

自動テスト済みなので下には出していない: 14.1, 14.2, 14.3, 14.4, 14.5, 14.6, 14.7

- [ ] **14.8** Press `Ctrl+F`, type a word → Every match is tinted in the text and the one at the caret is tinted stronger; the bar shows "i / n", or "No results" 〔未訳〕
- [ ] **14.9** Press `Ctrl+F` with a word selected; press `Esc` → The word is in the field; `Esc` closes the bar and the highlights go 〔未訳〕
- [ ] **14.10** With the bar focused type text; click into the document and type → While the bar has the focus nothing goes into the document; after the click it does 〔未訳〕
- [ ] **14.11** Turn on the Japanese IME and type `かな` into the field, press `Enter` while converting, then `Enter` again → The first `Enter` only confirms the conversion; the second steps to the next match 〔未訳〕
- [ ] **14.12** Open two files as tabs, search in one, switch to the other and back → Each tab shows its own count and highlights; the bar's query stays 〔未訳〕
- [ ] **14.13** Replace all in a CRLF file with a BOM, save, and look at the file → The line endings and the BOM are as they were 〔未訳〕
- [ ] **14.14** Look at the bar in the light and the dark theme, in a narrow window and with a long query → The bar uses the theme's colours, nothing overlaps, the buttons are the shared ones with the focus ring 〔未訳〕
- [ ] **14.15** In a document where a word occurs several times, press `Ctrl+H`, type the word and a replacement, press `Ctrl+Alt+Enter`, then `Ctrl+Z` once → Every place is replaced and the status bar says how many; one `Ctrl+Z` brings all of them back 〔未訳〕
- [ ] **14.16** With a match selected, press `Enter` in the Replace field repeatedly → Each press replaces that match and moves to the next one 〔未訳〕
- [ ] **14.17** Type a long query one letter at a time in a large file → Typing never waits; the count and the highlights follow the newest query 〔未訳〕

## 15. Rendered preview

自動テスト済みなので下には出していない: 15.1, 15.2, 15.3, 15.4

- [ ] **15.5** Click a line of the preview → The caret goes to that place in the source (never inside a character) 〔未訳〕
- [ ] **15.6** Press `Ctrl+F` and type a word → The matches are tinted in the preview too 〔未訳〕
- [ ] **15.7** Open an empty file with the preview on → The pane says there is nothing to preview 〔未訳〕
- [ ] **15.8** Type in a large file with the preview open → Typing never waits; the preview catches up 〔未訳〕
- [ ] **15.9** Look at the pane in the light and the dark theme, with long Japanese lines, a wide table and a code block, and while resizing → Lines wrap at the pane's width; colours are the theme's; nothing overlaps 〔未訳〕

## 16. Minimap

自動テスト済みなので下には出していない: 16.1

- [ ] **16.2** Click and drag in the strip → The source scrolls there; no caret is put by the click; hovering shows the lines around the pointer 〔未訳〕
- [ ] **16.3** Narrow the window, or open the preview until the source is narrow → The strip goes away and the text takes the room 〔未訳〕

## 17. Contents column

自動テスト済みなので下には出していない: 17.1

- [ ] **17.2** Click an entry → The preview scrolls to that heading and the caret goes to its start; the source follows 〔未訳〕
- [ ] **17.3** Narrow the preview (drag its edge), or open a document with no headings → The column goes away 〔未訳〕

## 18. Word wrap

- [ ] **18.1** Open a document with long lines in a narrow window and press Alt+Z → Long lines fold at the pane's width; the status line announces it; line numbers sit at each line's first row 〔未訳〕
- [ ] **18.2** With wrap on, type, select across folded lines, search, and click in a lower row → The caret, selection and match bands follow each row; the click puts the caret in that row 〔未訳〕
- [ ] **18.3** Press Alt+Z again, then restart the app → Lines scroll sideways again; the last choice is kept across restarts 〔未訳〕

## 19. Change marks in the gutter

- [ ] **19.1** Open a file, type a new line, change a line, delete a line, in the light and dark themes → A green bar by the added line, a blue bar by the changed one, a small red wedge where lines were taken out 〔未訳〕
- [ ] **19.2** Save (Ctrl+S) → The marks go; they come back only for what you edit afterwards 〔未訳〕
- [ ] **19.3** Undo back to the text as read → The marks go 〔未訳〕

## 20. Changes made to the file by another program

- [ ] **20.1** Open a file, change nothing, change the file from another program (another editor, `echo >> file`) → Within a moment the text follows the file and the status line says it was reloaded; the caret stays; Ctrl+Z brings back the earlier text 〔未訳〕
- [ ] **20.2** Type something in weft, then change the file from another program → A box names the file, counts the lines (+ the file's, - the window's) and lists a few of them 〔未訳〕
- [ ] **20.3** In the box press Esc → Your text stays and stays changed; the same file contents do not ask again; a different change asks again 〔未訳〕
- [ ] **20.4** In the box press Enter → The file's text goes in as one undo step; Ctrl+Z brings your text back; the document still counts as changed 〔未訳〕
- [ ] **20.5** Save with Ctrl+S → No box (the save is not taken as someone else's change) 〔未訳〕
- [ ] **20.6** Change the file twice while the box is open → The box shows the latest contents 〔未訳〕

## 21. Autosave and recovery

- [ ] **21.1** Type in a file without saving, wait 3 seconds, end the process (Task Manager) → `recovery.toml` in the state folder holds the file's path and your text 〔未訳〕
- [ ] **21.2** Start weft again → A box lists the file; Enter brings the text back as unsaved changes (Ctrl+Z returns the file's text), Esc leaves it 〔未訳〕
- [ ] **21.3** Quit normally (even choosing not to save) → `recovery.toml` is empty; the next start asks nothing 〔未訳〕
- [ ] **21.4** Save the file, wait 3 seconds → The file is no longer in `recovery.toml` 〔未訳〕

## 23. Paste a URL over a selection

- [ ] **23.1** Select a word, paste a copied `https://…` address → The word becomes `[word](https://…)`; Ctrl+Z restores the word 〔未訳〕
- [ ] **23.2** Paste the same address with nothing selected → The address is pasted as is 〔未訳〕

## 22. Image paste

- [ ] **22.1** Copy a picture (Win+Shift+S), press Alt+Shift+V in a saved note → `assets/<date>-<n>.webp` appears next to the note and `![](assets/…)` is at the caret; the status line says so 〔未訳〕
- [ ] **22.2** Press Alt+Shift+V again with the same picture → The same file is linked; no second file 〔未訳〕
- [ ] **22.3** Press Ctrl+Z → The link is gone in one step 〔未訳〕
- [ ] **22.4** Copy text only, press Alt+Shift+V → A notice: the clipboard has no picture 〔未訳〕
- [ ] **22.5** In a note without a file, press Alt+Shift+V → A notice that the document has no file yet 〔未訳〕

## 24. Folding headings

- [ ] **24.1** Put the caret in a section's body and press Alt+F → The body lines disappear; the heading shows `… <lines hidden>` and holds the caret 〔未訳〕
- [ ] **24.2** Press Alt+F on the folded heading → The lines come back 〔未訳〕
- [ ] **24.3** Fold a section, then press Down from its heading → The fold opens and the caret is on the next line 〔未訳〕
- [ ] **24.4** Fold a section, add a heading above it → The section stays folded 〔未訳〕
- [ ] **24.5** Run "Fold all sections" and then "Unfold all sections" from the palette → Every section folds (the top-level headings show) and then all open 〔未訳〕

## 25. Lint

- [ ] **25.1** Open a note with a skipped heading level (`#` then `###`), a table row with too few cells and an unclosed ``` fence → Each is underlined: the heading with a thin line, the table row and the fence with a thick one 〔未訳〕
- [ ] **25.2** Press F8 → The caret goes to the next finding and the status line names it with the total; at the last one it wraps to the first 〔未訳〕
- [ ] **25.3** Press Shift+F8 → The caret goes to the previous finding 〔未訳〕
- [ ] **25.4** Run "Fix all lint findings" from the palette, then Ctrl+Z once → The fixable ones are mended together; one undo restores the text 〔未訳〕
- [ ] **25.5** Look at it in the light and the dark theme → The underlines read in both 〔未訳〕
- [ ] **25.6** In a saved note, write `[a](gone.md)` with no such file next to it, and a link to a file that exists → Only the missing one is underlined; F8 names it "リンク先がありません" 〔未訳〕

## 26. Focus view

- [ ] **26.1** In a note with several `##` sections, put the caret in one and press Alt+Shift+F → Only that section (heading to the next heading of the same or higher level) shows 〔未訳〕
- [ ] **26.2** Move the caret down into the next section → The view follows: the next section shows, the old one is hidden 〔未訳〕
- [ ] **26.3** Press Alt+Shift+F again → All lines show again 〔未訳〕
- [ ] **26.4** Put the caret before the first heading and turn it on → Only the preamble shows 〔未訳〕
- [ ] **26.5** Look at it in the light and the dark theme → Same layout, line numbers keep their real values 〔未訳〕

## 27. Big files (8 MiB and more)

- [ ] **27.1** Make a file with `python scripts/make-big-md.py big.md 150000` and open it → It opens at once, read-only, with a notice that Ctrl+E edits it in your editor; the status bar shows "1 / N 行" and UTF-8 〔未訳〕
- [ ] **27.2** Press PageDown, ↓, PageUp, then End and Home → The view moves by page and line; End shows a full last page 〔未訳〕
- [ ] **27.3** Wheel, and click or drag on the bar at the right → The view follows; the thumb shows the place 〔未訳〕
- [ ] **27.4** Press Ctrl+E after scrolling → Your editor opens the file at the line on top 〔未訳〕
- [ ] **27.5** Append a line to the file from another program → The line count grows; no "reloaded" note 〔未訳〕
- [ ] **27.6** Look at it in the light and the dark theme → The same layout; line numbers readable 〔未訳〕

## 28. Lint settings

- [ ] **28.1** Write `[lint]` / `info = true` in config.toml, open a note with a line ending in three spaces → A dotted, quiet line under the spaces; F8 reaches it 〔未訳〕
- [ ] **28.2** Set `spacing = "space"` and write `日本語ABC日本語` → Dotted lines under the places that touch Latin text 〔未訳〕
- [ ] **28.3** Set `info = false` (or remove it) while the note is open → The dotted lines go; the warnings and errors stay 〔未訳〕
- [ ] **28.4** Write `tone = "x"` → A notice says the value is not one of auto, polite, plain; the default is used 〔未訳〕
