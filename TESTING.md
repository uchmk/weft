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
| 1.5 | Look at the window before anything is open | An empty-state ghost in the middle ("Hello from weft." over "Report a problem (F12)"), with no button under it |
| 1.6 | Look at `weft.exe` in Explorer and on the taskbar | The same woven icon as in the title bar |
| 1.7 | Press `Ctrl+Q` | The window closes; started again, it comes back at the place it closed at |
| 1.8 | Run `weft --keys`, then `weft --keys "<F1>" --more` | weft says `--keys needs a script`, then ``unknown argument `--more` ``, and does not start |

## 2. Key help

| # | Do | Expect |
|---|---|---|
| 2.1 | Press `F1` | A card over the dimmed window lists the keys in groups: "Everywhere" (settings, key help, quit), "Always" (F12, the palette, the scale) and "This list" |
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
| 4.1 | Press `Ctrl+Shift+P`, or click the palette field in the title bar | The palette lists Report a problem, Reset the scale, Settings, Key help and Quit |
| 4.2 | Look at the right of each row in 4.1 | Every command the keymap binds shows its key (`Ctrl+,`, `F1`, `Ctrl+Q`) |
| 4.3 | Type `set` and press `Enter` | The settings open |
| 4.4 | Press `Esc` with the palette open | The palette closes and nothing runs |

## 5. Notices

| # | Do | Expect |
|---|---|---|
| 5.1 | Put a line weft does not know in `keymap.toml` (an action named `nothing`) and start weft | The bell shows 1 unread; opening it, a "check" notice says `nothing` is no action of weft, and the other keys still work |
| 5.2 | Break `common.toml` (an unclosed `[`) while weft is open | A notice under the bell says what could not be read; the window keeps its theme and language |
| 5.3 | Press "Mark all read" | The bell's count goes away |

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
