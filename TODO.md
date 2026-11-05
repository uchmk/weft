# TODO

雛形（ito の `templates/app`）から作った。雛形の配線は、窓（場所を覚える）、common.toml のテーマ・言語・倍率、
`--version` / `--help`、F12 の不具合報告、`weft.com`、アイコンと版の情報、`--keys`、keymap.toml、F1 のヘルプ、設定の画面、パレット、字体まで済んだ。
下の `[ ]` は、編集の画面・保存ができてから配線するもの。自前で書かずに ito のクレートを使う
（足りない所は ito に頼む。`scripts/ito-first.py` が見ている）。
見た目の現状（撮った画面・直書き・直すべき 3 点・ito への頼みごと）は [docs/visual-baseline.md](docs/visual-baseline.md)。

- [x] `--keys` の台本: `ito-keyscript` の `Runner`（`State`・`Shot`・`Theme`・`Lang`・`Read`・`Quit`）。`WEFT_KEYS_DONE` は `lanes.conf` の `keys_env` と同じ。
  状態のファイルは `screen`（main・settings・help）・`lang`・`page`・`palette`・`notices`。
- [ ] `--keys` の残り: `ito-a11y`（`egui` の機能）を 0.2.4 で入れ、`<Announced:>` は TESTING 7.4 のテストで確かめた（保存・保存できない・元に戻すものが無い）。
  `<Tree:名前>`・`<Focused:>`・`<Enabled:>` は accesskit から引けるはずだが、まだ確かめていない。通知は `Cargo.toml` の機能 `test-hooks`
  （`ito-notify/test-sink`）を入れたビルドで `ITO_NOTIFY_SINK=<file>` に記録できる（TESTING 7.5）。出荷するビルドには入れない。
- [x] `keymap.toml` と設定の「キー」ページ: `ito-keymap`・`ito-prefs` の `keys_page`。既定のキーは `defaults/keymap.toml`
  （`<C-,>` 設定・`<F1>` ヘルプ・`<C-q>` 終了）。keymap.toml には変えた分だけを書く。読めない行はベルに出す。
- [x] F1 のヘルプ: `ito-help`。`?` は文字の入力に使うので割り当てない（設計の原則）。
- [x] 通知と設定の「通知」ページ: ページ（`notify_page`、試しの通知）と、保存できなかったときの `notify::tell`（`take_files`）。
- [x] 設定の画面: `ito_prefs::show`（一般・テーマ・キー・通知・詳細・情報）。`Ctrl+,` で開き、`Esc` で閉じる。
- [x] 窓の場所を戻す: `window::Saved` で戻し、`Tracker::follow` で書き、`on_exit` で `Tracker::close`。
- [x] 通知を入れる（0.2.4）: common.toml・テーマ・config.toml の `[notify]`・keymap.toml の読めなかったことは、ベルに `Kind::Warn` で 1 回だけ（直ってからまた壊れたら再び）。
  通知は飛び先のファイルを持つ（`Notice<PathBuf>`）。窓が後ろのときの保存の失敗は ito-prefs の `tell_open`（tag は通知の番号）で出し、押すと窓が前に出て文書が前に来る。
  別のエージェントが確かめた（verify 56 件、TESTING 5.4・5.5・7.3・7.4 はテスト、5.6・7.5 は人）。最小化した窓が戻らないのは ito-prefs の不具合だった（uchmk/ito#172、v0.232.0 で直り 0.2.6 で入れた）。
- [x] コマンドパレットの中身: 不具合の報告・拡大率を戻す・設定・キーのヘルプ・終了と、`commands::ALL` のうち `palette` のもの（保存・外部エディター・元に戻す・構造のコマンド）。キーは `ito-keymap` から引く。
- [x] 字体: `ito-fonts` で日本語の字体を別スレッドで読み、`egui` に足す。
- [x] `TESTING.md` に確認の行（起動・キーのヘルプ・設定・パレット・通知・拡大率・台本・編集・外部エディター）を足し、`cargo run --example make-testcheck` で `TESTING-CHECKS.md` を作った。
- [x] `icon.rs` に本当のアイコン（織り目。縦糸と横糸が交互に上下する）。`.exe` のアイコンも同じ絵から `build.rs` で作る。
- [x] リリースの workflow: `scripts/lanes.sh sync` が ito の雛形から書く（`lanes.conf` の `workflows=`）。

## weft の機能（フェーズ順。設計は [docs/design.md](docs/design.md)）

フェーズ 1〜3 が済めば、既存のエディターと違う価値を持つ。ここまでを最初の目標にする。
ワーカーへの分離（版番号つきの依頼、古い結果を捨てる）は、フェーズ 1 から設計に入れる。

### 着手前（設計の「進め方」の (3)・(4)）
- [x] 編集バッファ（rope など）と元に戻すを、ito・kura が既に持っていないか調べる → どちらにも無い（kura の `UndoStep` はファイル操作の元に戻す）。
- [x] 編集バッファ・位置・選択・トランザクションの元に戻すを ito に頼んだ（uchmk/ito#156）。`ito-textbuf` として ito の 0126b34（v0.222.0）に入り、フェーズ 1 で使っている（`src/engine.rs` は `ito-textbuf` の上の薄い層）。
- [x] コマンドエンジンの設計: 「対象.動作」の名前を `src/commands.rs` の `ALL` 1 か所に置き、キーマップ・パレット・ヘルプがそこから引く。文書のコマンドは `commands::edit`（UI を持たない）。
  ツールバーはまだない（作るときも `ALL` から出す）。

### フェーズ 1 編集エンジン（Document Engine）
- [x] バッファ・カーソル・選択（文字境界、書記素クラスタ）: `ito-textbuf` の `Doc`・`Selection`。`src/engine.rs` の `Editor` が束ねる。
- [x] 位置モジュール: `ito_textbuf::pos` の 1 か所だけで変換する。CRLF・全角・絵文字のテストは `engine.rs`。
- [x] 元に戻す・やり直す（1 操作 = 1 トランザクション）: `ito-textbuf` の履歴。打った文字は続けて 1 歩、コマンドの前後で区切る。
  ファイルへの作用の逆操作は、それを使う画像の貼り付け（フェーズ 4）で入れる。
- [x] 構造木: `src/structure.rs` が pulldown-cmark の `into_offset_iter` を `ito_outline::options()` で読む。ワーカーで版番号つきに作り、古い版は捨てる。
- [x] `ito-mdedit` の `Change` をトランザクションとして適用する所（`Editor::structure`、1 回で 1 歩、カーソルは `Change` の示す所へ）。
- [x] 名前つきコマンドとキーマップ（`ito-keys`・`ito-keymap`、`defaults/keymap.toml` の `app`・`edit` の層）、パレット。
- [x] 色づけ: 見出し・強調・コード・リストの記号は構造木と同じワーカーで、` ``` ` の中は `ito-syntax` の `Stream::for_fence`。追いつくまでは前の色のまま文字だけ出す。
- [x] 改行コード・BOM を読んだまま保存（一時ファイル → 置き換え。ファイルのスレッドで書く）。UTF-8 以外は警告して読み取り専用で開く（変換はしない）。
- [x] 1 キーで `$EDITOR` へ（`Ctrl+E`、`ito_openin::Config::open_file_at` に行・列。未保存なら先に保存）。列は uchmk/ito#158 で ito に入った（745ccb1）。
- [x] 未保存の変更があるときの終了は `ito-confirm` で聞く（やめる・保存せずに終了・保存して終了）。保存が済んでから閉じる。
- [x] ito に頼んだもの（ito の f58b51f で入り、weft の回り道を外した）:
  - uchmk/ito#159 `ito-textbuf`: 行の索引（`Buffer::line_of`・`line_range`・`line_col`・`byte_at`）、元に戻すで保存した所に戻ったら未保存でなくなる（`Doc::mark_saved`・`is_dirty`）、
    続けて消した文字を 1 歩にまとめる（`Doc::deleted`）。
  - uchmk/ito#160 `ito-mdedit`: リストの項目 1 つだけを上下に動かす（`move_item`。リストの端では `move_block` に任せる）。
  - uchmk/ito#161 `ito-outline`: `at_byte` が本文の末尾のカーソルも拾う（1 バイト前で聞く回避を外した）。

### フェーズ 2 日本語特化
- [ ] `ito-ime` の統合。確定の前後でカーソルと選択を保つ。
  変換中の文字をカーソルの所に出す・確定を入れる・候補の窓をカーソルに寄せる・変換中のキーをキーマップに渡さない、までは `src/view.rs` で配線済み。
- [ ] 入力の優先順位（IME → 補完 → キーマップ → コマンド）。変換中は補完を開かない、確定の Enter をコマンドにしない。
- [x] 全角スペースと意図しないハード改行を控えめにハイライト（全角スペースは薄い枠、段落の中のハード改行になる行末の空白は薄い色と点。TESTING.md 8.10）。
- [ ] 状態欄: 行・列、日本語の文字数、選択の文字数、保存状態、文字コード、改行コード、IME の状態（`ito-statusbar`）。
  IME のオン・オフと入力モードは uchmk/ito#166 で頼み、ito の v0.227.0（3f57ba3）で入った（`Ime::status(ctx) -> Option<Status>`、テスト用に `Ime::fake_status`）。
  0.2.5 で文字数（ワーカーで数え、選択は索引から両端だけ）・選択の文字数・文字コード・BOM・改行コード・変換中が入った（別のエージェントが 8 MB の実物と計測で確かめた。
  Ctrl+A は索引があれば 0.04 ms、無ければ「選択 …」）。
  0.2.6 で IME のオン・オフと入力モード（ito v0.232.0 の `Ime::status`。オフは薄く、ほかは「IME オン」。TESTING 8.14 はテスト、8.15 は人）が入り、
  文字数は `ito_textbuf::pos::count_chars`（範囲を書記素の境で広げてから。別のエージェントが絵文字・NFD・国旗・CRLF の全部の端で旧と同じ数を確かめた）になった。
  残りは桁区切り（`ito_common::text::grouped`）。入れると、今のテスト（`a_large_selection_is_counted_from_the_reading_or_shown_pending`）が期待する「選択 12345」が「選択 12,345」に変わる。
  ユーザーが区切りを入れる案を選んだ（2026-11-03）ので、文字数と選択の文字数に区切りを入れ、このテストの期待値も区切りのある形に変える。

### フェーズ 3 Markdown 操作（`ito-mdedit` を配線する）
- [x] `block.move_up` / `block.move_down`（リストの項目は `move_item` で子の項目ごと 1 つ、それ以外は `move_block`。`Alt+↑` / `Alt+↓`）、節の移動（`move_section`、`Alt+Shift+↑` / `Alt+Shift+↓`）。
- [x] `section.promote` / `section.demote`（`promote_section` / `demote_section`。子孫の見出しごと。`Alt+Shift+←` / `Alt+Shift+→`）。
- [x] リストの階層（`indent_item` / `outdent_item`、`Alt+→` / `Alt+←`）、`list.toggle_task`（`toggle_task`、`Ctrl+Enter`）、番号の振り直し（`renumber`、パレットから）。
- [x] `table.format`（`format_table`。全角は 2 セル幅）、`table.next_cell`（`next_cell`、Tab / Shift+Tab）。
  コマンドとパレットには入れた。Tab / Shift+Tab の割り当てが残っている。決めたこと（2026-11-03）: 表の中はセルの移動、リストの項目では字下げ・戻し（`indent_item` / `outdent_item`）、
  それ以外では Tab が空白を入れ、Shift+Tab が行頭の空白を戻す。空白の数は設定で選べる（既定は 4）。
  枝 `wt/tab` で作り、別のエージェントが確かめた（verify 58 件が通る。設定の画面・IME の変換中・元に戻す・読み取り専用も確かめた）。
  ただし表で Tab を押すと、ito-mdedit の `table::render` が行の字下げと CRLF を落とす（リストの中の表がリストの外に出る、CRLF のファイルに LF が混ざる）。
  前から `table.format` にあった不具合だが、Tab で毎回起きるので uchmk/ito#170 が入るまで取り込まない。
  #170（表の行頭と CRLF を保つ）と #171 は ito の v0.231.0（aef9deb）で入った。`wt/tab` で ito を aef9deb に上げ、表・リストの 5 つのテストが f58b51f で落ち aef9deb で通るのを確かめた（verify 70 件）。
  追加で決めたこと（2026-11-03）: 複数行を選んだ Tab は、選択の行それぞれを字下げする（リストの項目は子ごと `indent_item`、それ以外は行頭に空白）。Shift+Tab は逆。1 回の元に戻すで戻る。
  設定は空白の数（2・4・8）に加えて、空白の代わりにタブ文字を入れるかも選べる。
  `wt/tab`（c15ad87）で複数行を作ったが、別のエージェントが確かめて取り込まないことにした:
  - 元に戻らない: `- a\n\npara\n` で Tab → Shift+Tab が `- a\n\n  para\n` になり、その先は動かない（Tab は `para` を 4 動かすので項目の子になる）。TESTING 8.16 の書き方では落ちる。
  - 遅い: 項目ごとに全文を解析する。1 MB の末尾の 1000 項目で、Tab 4.4 秒・番号つき 9.6 秒・動かない Shift+Tab 8 秒。どれも UI のスレッドで動く。
  - 番号が 10 以上の項目: 字下げ・字上げが自分の印の幅で動く（ito-mdedit の `shift_item`）。`10. x10` で Tab → Shift+Tab が ` 1. x10` になる。カーソル 1 つの Tab と `Alt+→` でも起きる。
  複数行の規則（約 150 行、ito-mdedit の規則の写し）は ito-mdedit に `shift_lines` として uchmk/ito#179 で頼んだ（1 回の解析・元に戻ること・囲みを切らないこと）。
  番号の不具合は uchmk/ito#178。両方が ito の v0.236.0（1e43944）で入った（ito のセッションの知らせ）。`wt/tab` を `shift_lines` に替えて確かめ直す。
  `wt/tab`（e3cc756）を `shift_lines` に替え（weft の写しの規則 約 200 行を消した）、別のエージェントが確かめた: weft の側は不具合なし（verify 110 件、
  1 MB の 1000 項目で Tab 8 ms・Shift+Tab 5 ms、CRLF・BOM・混在の往復でバイトが一致、読み取り専用・幅 2/4/8・タブ文字・表のセル移動）。
  ただし ito の側の不具合で取り込みを待つ:
  - 選択がコードの囲みの途中まで掛かると、幅 4（既定）・8・タブ文字で Tab → Shift+Tab の後に閉じの行が字下げのまま残り、囲みが閉じずに文書の残りを飲み込む（幅 2 だけ往復する）。
    TESTING 8.16 の本文も Tab×2 → Shift+Tab×2 で元に戻らない。最上位のリストの最初の項目は Tab 2 回で字下げのコードになる。uchmk/ito#182 で頼んだ。
  - 表のセルの移動（`next_cell`）と `table.format` が、コードの中の表のような文字を整形して書き換える（Tab で踏みやすくなる）。uchmk/ito#183 で頼んだ。
  ユーザーが決めたこと（2026-11-03）: 幅 4 で段落の行に Tab を押すと字下げのコードになるのは、Markdown の仕様どおりとしてそのままにする。
  入ったら ito を上げ、`main`（0.2.8 と `wt/keys`）を取り込んで、8.16・8.17 の行の番号を `wt/keys` の行とぶつからないように振り直す。
  #182・#183 は ito の v0.240.0（f1f3b75）で入り、0.2.9 で main の ito を v0.242.0（78ba28d）に上げた。次は `wt/tab` に main を取り込んで確かめ直す
  （8.16 は 2 回目の Tab が何もしなくなる。行の番号は `wt/keys` の 8.20〜8.23 とぶつかる）。
  2026-11-05: `wt/tab` を `main` に取り込んだ（ito は v0.242.0 のまま。Tab / Shift+Tab は `edit.tab` / `edit.untab`）。
  TESTING の行は 8.24〜8.30 に振り直した（旧 8.16・8.17・8.20〜8.24。テストの `/// 8.NN:` も同じ）。
  ito の `shift_lines` は、Shift+Tab で戻せない Tab（段落の下で項目がコードになる 2 回目の Tab など）を何もしない（`None`）ことにした。
  それに合わせて、2 回目の Tab を期待していたテスト 3 つ（8.24・8.29）と TESTING 8.24 の言葉を直した。
- [ ] 見出しの折りたたみ（`heading.fold`）、目次の挿入・更新（`heading.insert_toc`）。
  折りたたみは `ito-outline` の `section` を使う weft の表示。目次の挿入・更新は uchmk/ito#163 で頼み、v0.230.0（3ce9f13）の ito-mdedit `insert_toc` で入った。
  0.2.8 で目次の挿入・更新が入った（パレットから。1 回の元に戻すで戻る、最新なら何もしない。TESTING 11.1）。残りは折りたたみ。
- [ ] 構造選択（単語 → 段落 → リスト → 節 → 親の節）、構造を壊さない削除、構造ナビゲーション、構造診断（lint）、集中表示。
  構造選択・削除・ナビゲーションは uchmk/ito#163、lint は uchmk/ito#164 で頼み、v0.230.0（3ce9f13）で入った。
  ito-mdedit の `expand_selection` / `shrink_selection`・`delete_heading`・`next` と、新しいクレート ito-mdlint（`lint` の `Finding` を `fix_all` で 1 つの `Change` にする）。
  `shrink_selection` は状態を持たないので、広げた経路どおりに戻すには weft の側で選択のスタックを持つ。集中表示は `ito-outline` の `section` を使う weft の表示。
  0.2.8 で構造選択（`select.expand` / `shrink`。weft の側の選択のスタック）・見出しの削除 4 通り・構造ナビゲーション 12 個が入った（どれもパレットから。
  TESTING 11.2〜11.9。別のエージェントが確かめた、verify 95 件）。
  キー（2026-11-03、ユーザーが推奨の案を選んだ）: 広げる `Alt+W`・狭める `Alt+Shift+W`・次・前の見出し `Alt+N` / `Alt+P`・親の見出し `Alt+U`。
  目次と見出しの削除はパレットのまま。Alt のキーなので `wt/keys` で入れる。
  既知の不具合: 引用・リストの中や字下げした見出しの行頭にカーソルがあると、削除が直前の見出しに効く（ito-outline の `Heading.range` が `#` から始まるため）。
  uchmk/ito#181 で頼み、ito の v0.239.0（b0049a3）で入った。0.2.9 で直った（別のエージェントが `> `・`- `・字下げ × `#`・`##` で 4 つの削除を確かめた。
  `heading.delete` は前置きごと消す。CRLF の文書の目次は CRLF）。構造のキーも 0.2.9 で入った。残りは lint と集中表示。
- [x] 書式のキー（2026-11-03 にユーザーが決めた）: 行内のコードは `Ctrl+``、コードの囲み（` ``` `）は `Ctrl+Shift+``。太字・斜体・取り消し線・リンク・見出し 1〜6・引用・
  箇条書き・番号・タスクも名前つきのコマンドにする。どれも選択の範囲に効き（行ごとに包む、全部が済んでいれば外す）、1 回の元に戻すで戻る。
  タスクは選択の範囲の項目をまとめてチェックするキーと、まとめて外すキーを持つ。中身（選択を受けて選択を返す、コードの中では何もしない、CRLF・字下げ・`>` を保つ）は
  uchmk/ito#174 で ito-mdedit に頼み、ito の v0.232.0（10f9abe）の `format`（`inline`・`line_type`・`check_tasks` / `uncheck_tasks` / `toggle_tasks`、`SelChange`）で入った。
  0.2.7 で名前つきのコマンド（`inline.bold` / `italic` / `code` / `strike` / `link`・`heading.set_1`〜`6`・`heading.clear`・`block.quote` / `fence`・
  `list.bullet` / `number` / `task`・`list.check_tasks` / `uncheck_tasks`）と Ctrl のキーが入った。`Ctrl+Enter` は選択の全部のタスクを切り替える。
  CommonMark で効かない強調（`unflanked`）は状態欄に出して読み上げる。別のエージェントが本物の egui のキーの出来事で JIS・US の両方を通し、
  1 回の `Ctrl+Z` で全部戻ることも確かめた（verify 86 件）。ただし TESTING 10.12 のテストはキーの解決（`from_egui` → `resolve`）までで、
  `read_keys` を通して文書が変わるかは見ていない。`ito_keys::read` に替えるときに、本物のキーの出来事から文書の変化までを見るテストにする。
  残りは Alt のキー（引用・タスク・取り消し線・まとめてチェック／外す）。今はパレットからだけ呼べる。
  キーの決まっていないもの: ユーザーは日本語配列で、`` ` `` は `Shift+@` でしか打てない（`Ctrl+Alt+`` も同じ理由で不可）。どのキーにするかは実機で届き方を確かめてから決める（ユーザーの選んだ案）。
  実機（日本語配列 HKL 0x04110411、eframe 0.36.2・ito-keys aef9deb、スキャンコードで送った。ログは scratchpad の `keyprobe`）で確かめたこと:
  `Ctrl+Shift+@`（JIS で打つ「Ctrl+`」）は `<C-`>`。`Ctrl+@` と `Ctrl+[` はどちらも `<C-[>`。`<C-S-`>` は何を押しても届かない（ito-keys が記号の Shift を落とす）。
  `Ctrl+^` と `Ctrl+Shift+^` はどちらも `<C-=>`。`Ctrl+B` / `I` / `K`・`Ctrl+Shift+K`・`Ctrl+1`〜`6` はそのまま届く。
  `Ctrl+Shift+X`・`Ctrl+Alt+X` は egui の切り取りになり、weft の `read_keys` は `from_clipboard` を通さないので捨てられる。`Ctrl+Shift+Q` は egui の既定の
  `quit_shortcuts` が取って窓を閉じる（weft では終了の確認になる）。`Alt+X` は `<A-x>` に加えて文字の `x` も来る（`Alt+Shift+5` は `<A-5>` と `%`）。
  weft の `view.rs` は来た文字をそのまま入れるので、Alt＋文字のキーは文字も打ってしまう。ito-pane・ito-prefs はそれぞれ自前で落としているので、
  共通の読み方を uchmk/ito#176 で ito-keys に頼み、ito の v0.233.0（9f1da36）の `ito_keys::read`（キーと文字を来た順に返し、取ったキーと Alt の和音の文字を落とす）で入った。
  `Ctrl+Shift+Q` を使うなら egui の `quit_shortcuts` を空にする。一括チェック・一括解除は `Ctrl+Enter` / `Ctrl+Shift+Enter` 以外（ユーザーの指定）。
  決めたキー（2026-11-03、ユーザーが推奨の案を選んだ）: 行内のコード `Ctrl+Shift+@`（`<C-`>`）、コードの囲み `Ctrl+@`（`<C-[>`。`Ctrl+[` も同じ）、
  太字・斜体・リンク `Ctrl+B` / `Ctrl+I` / `Ctrl+K`、見出し 1〜6 `Ctrl+1`〜`6`（同じ見出しでもう一度押すと段落）、番号・箇条書き `Ctrl+Shift+[` / `Ctrl+Shift+]`（`<C-{>` / `<C-}>`）、
  引用 `Alt+Q`、タスク `Alt+T`、取り消し線 `Alt+Shift+5`（`<A-5>`）、まとめてチェック・外す `Alt+X` / `Alt+Shift+X`。`list.toggle_task` は `Ctrl+Enter` のまま。
  Ctrl のキーは 0.2.7 で割り当てた。Alt のキーは ito を v0.234.0 に上げて `ito_keys::read` に替えてから割り当てる。
  枝 `wt/keys`（6dcfed6、ito v0.236.0）で `ito_keys::read` に替えて Alt のキーを入れたが、別のエージェントが確かめて取り込まないことにした:
  IME の確定と後の文字の順が入れ替わる（`한 ` が ` 한`）、macOS の Option の文字（`å`・JIS の `\`）が落ちる、Alt＋テンキーの `¥`（Alt+0165）が取り消し線になり `¥` も落ちる。
  どれも `ito_keys::read` の中なので uchmk/ito#180 で頼んだ。入ったら ito を上げ、IME の確定を `read` の順で入れ、取り消し線を `<A-S-5>` にして確かめ直す。
  ito の v0.239.0（b0049a3）で入った（`Input::Ime`・`read_for(.., mac)`・数字キーが Shift を残す・Alt＋テンキーの `¥` を残す）。
  0.2.9 で入った（`wt/keys` d027591。取り消し線は `<A-S-5>`）。別のエージェントが本物の egui の出来事で確かめた（verify 106 件）:
  確定はキーの順に入り二重に入らない、変換中の文字は描かれる、Mac の割り当てのない Option は文字を打ち割り当てのあるものはコマンドだけ、
  1 フレームの Alt+0165 は `¥` を打ちコマンドを走らせない、`Alt+5` は何もしない、Alt のキーはどれも 1 回の元に戻すで戻り文字を打たない。
  10.14 の書き換えで弱めた確かめはない（取り消し線のキーの移動と桁区切りだけ）。

### フェーズ 4 実用エディター
- [x] 外部変更の検知（フォルダーを監視。未保存があれば差分を見せて選ばせる）。
- [x] 自動保存・強制終了からの復元（控えを `recovery.toml` に、起動時に戻すか聞く）。
- [x] 画像の貼り付け（`edit.paste_image`・Alt+Shift+V。`assets/` へ WebP、日時＋連番、同じ画像は既存のファイルを再利用）。残り: 2000px への縮小（ito#194 待ち）、元に戻すでファイルもゴミ箱へ、未参照の添付の掃除コマンド。
  非可逆 WebP の手段は ito・kura に無かった（kura は `image` で読むだけ、`image` の書き出しは可逆のみ。クリップボードの画像を読む部品も無い）。
  uchmk/ito#162 で頼み、ito の v0.228.0（c77aba9）の `ito-image` で入った（`clipboard_image()` で RGBA、透明を残す非可逆 WebP の `encode_webp`。
  テストでは `ito_image::fake` か `ITO_CLIPBOARD_IMAGE`）。4K ではエンコードが重いのでワーカーで呼ぶ。rev を上げたあと、フェーズ 4 の順で取りかかる。
- [ ] 貼り付けの賢さ（HTML・表 → Markdown）。選択に URL → `[選択](URL)` は 0.2.25 で済み。
- [ ] 文書のタブ（2026-11-03 にユーザーが頼んだ）: 1 つの窓で複数の `.md` を開いて切り替える。設定で、タブの出し方（いつも・2 つ以上のとき・出さない）、
  外から開いたファイルを動いている窓のタブにするか新しい窓にするか、起動時に前のタブを開き直すかを選ぶ。タブの型とタブの帯は ito に無い
  （michi・tsumugi はそれぞれ自前）ので uchmk/ito#175 で頼んだ。kura の opener から開いたファイルを動いている窓へ渡す所（`ito-ipc`）も同じ Issue に書いた。
  ito の v0.233.0（9f1da36）で入った: 新しいクレート `ito-tabs`（`Tabs<Id>`・`Op`・`labels`・`tabs.toml`・設定の `ShowBar` / `OpenFromOutside` / `Reopen`・
  コマンド名 `tab.next` / `tab.prev` / `tab.close` / `tab.goto_1`〜`9`・`bar`）と `ito_ipc::single::acquire`（`OpenRequest::parse`）。
  設定の画面の行（ito-prefs）はまだ無いので、3 つの行を ito のセッションに頼んだ（2026-11-03、uchmk/ito#177。`ito_prefs::tabs_rows(ui, look, &mut ito_tabs::Settings) -> bool`、
  機能 `tabs`。値はアプリごとに weft の設定の書き手で保存し、common.toml には入れない）。ito の v0.234.0（4f40140）で入った
  （common.toml の言語に合わせるなら `tabs_rows_in(ui, look, TabsWords::of(lang), settings)`、設定の検索は `TABS_KEYS`・`tabs_index`。実機の `--keys` は ito では未確認）。
  2026-11-05: 枝 `wt/tabs`（ca1539a から。ito は b0049a3）の未コミットの下ごしらえ（`[tabs]` の読み書き `prefs.rs`・設定の行 `settings.rs`・
  タブのキー `defaults/keymap.toml`・コマンド名 `commands.rs`・`lang/*.toml`。+406/−94）は、タブの実体（`tabs.rs`・`App` への組み込み）が無く、
  コマンドが何もしない半端な状態なので main には入れず、枝と作業ツリーを消した。差分は `C:\dev\_wip\weft-tabs-settings.patch`（main の afc0938 には ito の rev と
  `prefs.rs` の差で素直には当たらない。タブを作るときに手で写す）。
  2026-11-05: 枝 `wt/doctabs` でタブの実体を作った（`tabs.rs`。開く・切り替え・閉じる・タブごとの保存と未保存の確認・終了の確認・前回のタブの復元・帯・設定の行。テスト 9 件）。
  2026-11-05: 単一インスタンスを入れた（`door.rs`。「外から開くとき」が新しいタブのときだけ `ito_ipc::single::acquire`。新しい窓・`--keys` は取らない）。
  残り: `--keys` での実機の確かめ（TESTING に書く）。複数ファイルの引数。
- [x] 右に整形プレビュー（`ito-mdview`。0.2.15）。
- [x] 原文にミニマップ（`ito-minimap`。0.2.16）。
- [x] 目次（`ito-outline`）の欄（整形側。0.2.17）。
- [x] ガターの変更表示（`ito-diff`、変更の周りだけ計算。0.2.19）。
- [x] 検索と置換（`ito-find`・`ito-match`、正規表現。0.2.13）。入力しながらの絞り込み・あいまい検索・`[[…]]` は未。
- [ ] YAML フロントマターの扱い。
- [ ] 大きなファイルの 3 段階（約 1 MiB まで・1〜8 MiB・8 MiB 以上）。計測用に数十万行の `.md` を作るスクリプトと `--keys` の計測。

### フェーズ 5 vault
- [ ] 索引（見出し・リンク・全文。`ito-store`、変更のあったファイルだけ更新）。1 万ファイルの vault を作るスクリプトで計測。
- [ ] `[[…]]` の補完（`ito-match`・`ito-palette`）、画像・ファイルのパス補完。
- [ ] バックリンク、全文検索。
- [ ] リネーム・移動への追従（変える一覧を見せ、1 回で元に戻せる）。
- [ ] スナップショットの世代管理（`.snapshots/`）、添付の掃除、リンクのホバープレビュー。

### kura との接続
- [ ] kura の opener で `.md` に Enter → weft が開き、`+N` の行指定が効く。
  kura は `ito_openin::lines::line_arg` で行の渡し方を決める。組み込みの表に weft を足すのを uchmk/ito#165 で頼み、ito の v0.226.2（ceb61fd）で入った
  （`weft = joined("+", ":", false)`、列は文字数）。kura は rev を上げるだけ。
  weft の側は 0.2.3 で入った（`weft +行[:列] FILE`、列は文字数、範囲の外は端に寄せる。TESTING.md 9.4〜9.6 は自動、9.7 は人）。
  kura は v0.139.55（9ae7ed9）で ito を ceb61fd に上げ、yazi.toml に weft の opener と `*.md use = ["weft","edit","open"]` を足した（kura のセッションの知らせ）。
  weft が PATH に無いうちは kura が飛ばして nvim が開く。残りは weft.exe を PATH に入れること（`cargo install --path crates/weft`）と、TESTING 9.7 を人が確かめること。
  今の kura が行を渡すのは目次（Enter）の見出しだけで、列を渡す開き方はまだ無い。

### その後（設計の「後回し」）
- 数式（v2）・Mermaid（v2〜v3。ライブラリの選定は ito への crate-request の候補）、書き出し、動画、テンプレート、タスク・タグの一覧、
  日本語の整形ルール、見出しのナンバリング、ライブプレビューを作るかの判断。

## 見つけた課題（ユーザーが認めるまで手をつけない）
- 編集欄の等幅フォントで、全角の括弧・数字・英字（U+FF01〜FF5A）が egui の `emoji-icon-font` の字形で描かれる（日本語の書体を族の最後に足しているため）。
  全角スペース（U+3000）はメイリオから全角で出ている。また JetBrains Mono（0.6 em）に対し漢字は 1.0 em で 1.67 桁になり、`format_table` で整えた表が編集欄で揃わない。
  字の並びを ito で一か所にまとめるのを uchmk/ito#167 で頼み、v0.230.0（3ce9f13）の ito-fonts `load_in_background(ctx, mono)` で入った。0.2.6 で `take_font` を置き換えた。
- 文字数の桁区切り（`12,345`）の部品が ito-common に無いので uchmk/ito#168 で頼み、v0.229.0 の `ito_common::text::grouped` で入った。状態欄で使うのは残り（上の状態欄の項目）。
- 範囲の書記素を数える部品が ito-textbuf の `pos` に無いので uchmk/ito#169 で頼み、v0.229.0 の `pos::count_chars` で入った。0.2.6 で weft の `count::count` を置き換えた。
- ito-mdedit のリストの操作が、コードの囲みの中の `- x` を項目として扱う。番号つきの項目を字下げしても番号を振り直さない（CommonMark ではリストでなくなる）。
  タブで字下げした項目を項目と見ない。uchmk/ito#171 で頼み、v0.231.0（aef9deb）で直った（コードの中では `None`、字下げ・字上げで番号を振り直す、タブの字下げも項目と見る）。
  `list.indent` と Tab の両方に効く。
- 設定の画面でタブの幅を素早く 2 回変えると、書き込みのスレッドの順で古い値が残りうる（`prefs.rs` の `tab_width_edit`。`notify_edit` も同じ作り。コードを読んだだけ）。
- 最小化した窓で、保存できなかった通知を押しても窓が戻らない（ito-prefs の `notify_cfg.rs` が `ViewportCommand::Focus` だけを送る。`Minimized(false)` が要る）。
  eframe 0.36.2 だけの小さな再現でも確かめた。fumi・michi・fude も同じ作り。uchmk/ito#172 で頼み、v0.232.0 で直った。0.2.6 で入れた（TESTING 5.7 は人が確かめる）。
- 通知の残り（低）: 言語の誤りと `prefs.failed` は 1 回だけの知らせ（`news()`）を通らない。保存の失敗のトーストは失敗のたびに 1 つずつ積もる。
  トーストの本文にパスが 2 回出る（前から）。TESTING-CHECKS の人の行が 〔未訳〕（`scripts/testcheck-ja.toml` が無い）。
- ito-outline の `headings` が見出しごとに先頭から行を数えるので、文書の大きさの 2 乗で遅い（8 MB で 4.6 秒、行の多い文章では 71 秒。行数を続けて数えれば 21〜63 ms）。
  画面は待たないが、編集のあと状態欄の数が数秒「…」と古い数のまま。uchmk/ito#173 で頼み、v0.232.0 で直った。
  0.2.6 で入れ、8 MB（見出し 10,599）の 1 回の編集のあとの読み取りが 11.5 秒から 0.112 秒（見出し 11 ms）になった（release で計測）。
- 読み取りが追いついていない間、選択の文字数は「…」なのに、文書の文字数は古い数のまま印が無い（TESTING 8.12 では設計どおりとしている）。
- IME の状態は egui が画面を描くときにしか読まない。IME の切り替えでは再描画が起きないので、タスクバーのメニューなどで切り替えると、
  次にマウスを動かすか時計が変わるまで（最大 1 分）状態欄が古いままのおそれ（検証のエージェントがコードを読んだ。TESTING 8.15 で人が確かめる）。
  直すなら ito-ime が IME の変化で再描画を頼む形が要る（ito に頼む）。
- 構造のコマンド（0.2.8）の検証で見つけたもの（どれもコードを読んだか、テストで確かめた）:
  - 目次の挿入と構造のコマンドは UI のスレッドで全文を解析する。1 MB で数十 ms（目次 30〜70 ms）。原則 6 に照らすなら、ワーカーの構造木を使うか ito に速くしてもらう。
  - 目次には、カーソルのある節を囲む見出し自身が入らない（ito の仕様）。
  - `heading.to_paragraph` は `heading.clear`（0.2.7）と同じ結果になる。パレットに 2 つ並ぶ。
  - `cursor.*` で行き先が無いとき「ここでは何もしない」と読み上げる（「次の見出しはない」などの方が分かりやすい）。
  - F1 のヘルプはキーのあるコマンドしか出さないので、パレットだけのコマンドはヘルプから見つからない。
  - TESTING.md に 8.11 の行が無い（番号が飛んでいる）。新しい TESTING の行は TESTING-CHECKS で 〔未訳〕。
- キーの読み方（`wt/keys` の検証で見つけたもの。0.2.7 の main でも起きるものを含む）:
  - 取ったキーの予測をフレームごとにするので、まれに外れうる（コードを読んだだけ）。
  - F1 と文字が同じフレームに来たとき・普通の文字の後に和音の途中が残るとき、の扱い（小さい）。
  - パレットの検索欄で Alt+X を押すと `x` が入る（main でも起きる）。パレットの検索欄（ito-palette の TextEdit）が自分で文字を読むので、weft では直せない。
    直すなら ito-palette か ito-keys に頼む（まだ頼んでいない）。
  - 実機の Windows の Alt+テンキーで、Alt を放した後のフレームに文字が来ると、`¥` が落ちるおそれ（`wt/keys` の実装担当がコードを読んだ。TESTING 8.23 で人が確かめる）。
  - 韓国語などの確定と Enter が同じフレームに来ると、ito-ime が Enter を飲み込むおそれ（コードを読んだだけ）。
  - クリップボードのイベントを中身の値で突き合わせている（同じ中身の貼り付けが続くと取り違えうる。コードを読んだだけ）。
- Tab（`wt/tab`）の検証で見つけた、仕様として決めたほうがよいもの:
  - 1 行の中の選択で Tab を押すと、選んだ文字が空白に置き換わる（`hello world` の 2..7 で `he    orld`）。字下げにするか、今のままか。
  - 表の行だけにまたがる選択では、Tab も Shift+Tab も何もしない（今のテストがそう決めている）。
  - リストの項目の続きの行（`- b\n  more b` の `more b`）では、Tab が位置に空白を入れ、Shift+Tab は行頭を戻すので、打ち消し合わない。
  - 行の途中の Tab は次のタブ位置までではなく、いつも N 個の空白を入れる。
  - 子を持つ最初の子の項目で Shift+Tab → Tab をすると、後ろの兄弟が子になったまま戻らない（アウトライナーではよくある動き。ito#182 に参考として書いた）。
