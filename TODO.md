# TODO

雛形（ito の `templates/app`）から作った。雛形の配線は、窓（場所を覚える）、common.toml のテーマ・言語・倍率、
`--version` / `--help`、F12 の不具合報告、`weft.com`、アイコンと版の情報、`--keys`、keymap.toml、F1 のヘルプ、設定の画面、パレット、字体まで済んだ。
下の `[ ]` は、編集の画面・保存ができてから配線するもの。自前で書かずに ito のクレートを使う
（足りない所は ito に頼む。`scripts/ito-first.py` が見ている）。

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
  別のエージェントが確かめた（verify 56 件、TESTING 5.4・5.5・7.3・7.4 はテスト、5.6・7.5 は人）。最小化した窓が戻らないのは ito-prefs の不具合（見つけた課題）。
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
  Ctrl+A は索引があれば 0.04 ms、無ければ「選択 …」）。残りは IME のオン・オフと入力モード: ito の rev を上げて状態欄に配線する。実際の IME での表示は weft の側で確かめる。
  同じ rev で、文字数の数え方を `ito_textbuf::pos::count_chars`（#169、v0.229.0）に、桁区切りを `ito_common::text::grouped`（#168、v0.229.0）に置き換える。

### フェーズ 3 Markdown 操作（`ito-mdedit` を配線する）
- [x] `block.move_up` / `block.move_down`（リストの項目は `move_item` で子の項目ごと 1 つ、それ以外は `move_block`。`Alt+↑` / `Alt+↓`）、節の移動（`move_section`、`Alt+Shift+↑` / `Alt+Shift+↓`）。
- [x] `section.promote` / `section.demote`（`promote_section` / `demote_section`。子孫の見出しごと。`Alt+Shift+←` / `Alt+Shift+→`）。
- [x] リストの階層（`indent_item` / `outdent_item`、`Alt+→` / `Alt+←`）、`list.toggle_task`（`toggle_task`、`Ctrl+Enter`）、番号の振り直し（`renumber`、パレットから）。
- [ ] `table.format`（`format_table`。全角は 2 セル幅）、`table.next_cell`（`next_cell`、Tab / Shift+Tab）。
  コマンドとパレットには入れた。Tab / Shift+Tab の割り当てが残っている。決めたこと（2026-11-03）: 表の中はセルの移動、リストの項目では字下げ・戻し（`indent_item` / `outdent_item`）、
  それ以外では Tab が空白を入れ、Shift+Tab が行頭の空白を戻す。空白の数は設定で選べる（既定は 4）。
  枝 `wt/tab` で作り、別のエージェントが確かめた（verify 58 件が通る。設定の画面・IME の変換中・元に戻す・読み取り専用も確かめた）。
  ただし表で Tab を押すと、ito-mdedit の `table::render` が行の字下げと CRLF を落とす（リストの中の表がリストの外に出る、CRLF のファイルに LF が混ざる）。
  前から `table.format` にあった不具合だが、Tab で毎回起きるので uchmk/ito#170 が入るまで取り込まない。
  #170（表の行頭と CRLF を保つ）と #171 は ito の v0.231.0（aef9deb）で入った。`wt/tab` を main に載せ替え、ito を aef9deb に上げて確かめ直している。
- [ ] 見出しの折りたたみ（`heading.fold`）、目次の挿入・更新（`heading.insert_toc`）。
  折りたたみは `ito-outline` の `section` を使う weft の表示。目次の挿入・更新は uchmk/ito#163 で頼み、v0.230.0（3ce9f13）の ito-mdedit `insert_toc` で入った。
- [ ] 構造選択（単語 → 段落 → リスト → 節 → 親の節）、構造を壊さない削除、構造ナビゲーション、構造診断（lint）、集中表示。
  構造選択・削除・ナビゲーションは uchmk/ito#163、lint は uchmk/ito#164 で頼み、v0.230.0（3ce9f13）で入った。
  ito-mdedit の `expand_selection` / `shrink_selection`・`delete_heading`・`next` と、新しいクレート ito-mdlint（`lint` の `Finding` を `fix_all` で 1 つの `Change` にする）。
  `shrink_selection` は状態を持たないので、広げた経路どおりに戻すには weft の側で選択のスタックを持つ。集中表示は `ito-outline` の `section` を使う weft の表示。
- [ ] 書式のキー（2026-11-03 にユーザーが決めた）: 行内のコードは `Ctrl+``、コードの囲み（` ``` `）は `Ctrl+Shift+``。太字・斜体・取り消し線・リンク・見出し 1〜6・引用・
  箇条書き・番号・タスクも名前つきのコマンドにする。どれも選択の範囲に効き（行ごとに包む、全部が済んでいれば外す）、1 回の元に戻すで戻る。
  タスクは選択の範囲の項目をまとめてチェックするキーと、まとめて外すキーを持つ。中身（選択を受けて選択を返す、コードの中では何もしない、CRLF・字下げ・`>` を保つ）は
  ito-mdedit に頼む。`Ctrl+Shift+X` は egui が切り取りとして取るので取り消し線には使えない。

### フェーズ 4 実用エディター
- [ ] 自動保存・強制終了からの復元、外部変更の検知（`ito-watch`。未保存があれば差分を見せて選ばせる）。
- [ ] 画像の貼り付け（添付フォルダー `assets/` へ WebP、日時＋連番、ハッシュで重複を再利用、1 回の元に戻すでファイルもゴミ箱へ）。
  非可逆 WebP の手段は ito・kura に無かった（kura は `image` で読むだけ、`image` の書き出しは可逆のみ。クリップボードの画像を読む部品も無い）。
  uchmk/ito#162 で頼み、ito の v0.228.0（c77aba9）の `ito-image` で入った（`clipboard_image()` で RGBA、透明を残す非可逆 WebP の `encode_webp`。
  テストでは `ito_image::fake` か `ITO_CLIPBOARD_IMAGE`）。4K ではエンコードが重いのでワーカーで呼ぶ。rev を上げたあと、フェーズ 4 の順で取りかかる。
- [ ] 貼り付けの賢さ（選択に URL → `[選択](URL)`、HTML・表 → Markdown）。
- [ ] 右に整形プレビュー（`ito-mdview`）、原文にミニマップ（`ito-minimap`）、目次（`ito-outline`）、位置の同期。
- [ ] ガターの変更表示（`ito-diff`、変更の周りだけ計算）。
- [ ] 検索と置換（`ito-match`、正規表現・あいまい）。
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
  字の並びを ito で一か所にまとめるのを uchmk/ito#167 で頼み、v0.230.0（3ce9f13）の ito-fonts `load_in_background(ctx, mono)` で入った。rev を上げるときに `take_font` を置き換える。
- 文字数の桁区切り（`12,345`）の部品が ito-common に無いので uchmk/ito#168 で頼み、v0.229.0 の `ito_common::text::grouped` で入った。rev を上げるときに状態欄で使う。
- 範囲の書記素を数える部品が ito-textbuf の `pos` に無いので uchmk/ito#169 で頼み、v0.229.0 の `pos::count_chars` で入った。rev を上げるときに weft の `count::count` を置き換える。
- ito-mdedit のリストの操作が、コードの囲みの中の `- x` を項目として扱う。番号つきの項目を字下げしても番号を振り直さない（CommonMark ではリストでなくなる）。
  タブで字下げした項目を項目と見ない。uchmk/ito#171 で頼み、v0.231.0（aef9deb）で直った（コードの中では `None`、字下げ・字上げで番号を振り直す、タブの字下げも項目と見る）。
  `list.indent` と Tab の両方に効く。
- 複数行の選択にリストの項目が入っていると、Tab の働きが選択の終わりの行で決まる（終わりが項目ならその項目だけ字下げ、段落なら全部の行に空白）。
  選択の全部の行を字下げするか、どう揃えるかはユーザーが決める（TESTING 8.11 の文言も合わせる）。
- 設定の画面でタブの幅を素早く 2 回変えると、書き込みのスレッドの順で古い値が残りうる（`prefs.rs` の `tab_width_edit`。`notify_edit` も同じ作り。コードを読んだだけ）。
- 最小化した窓で、保存できなかった通知を押しても窓が戻らない（ito-prefs の `notify_cfg.rs` が `ViewportCommand::Focus` だけを送る。`Minimized(false)` が要る）。
  eframe 0.36.2 だけの小さな再現でも確かめた。fumi・michi・fude も同じ作り。ito に頼む。
- 通知の残り（低）: 言語の誤りと `prefs.failed` は 1 回だけの知らせ（`news()`）を通らない。保存の失敗のトーストは失敗のたびに 1 つずつ積もる。
  トーストの本文にパスが 2 回出る（前から）。TESTING-CHECKS の人の行が 〔未訳〕（`scripts/testcheck-ja.toml` が無い）。
- ito-outline の `headings` が見出しごとに先頭から行を数えるので、文書の大きさの 2 乗で遅い（8 MB で 4.6 秒、行の多い文章では 71 秒。行数を続けて数えれば 21〜63 ms）。
  画面は待たないが、編集のあと状態欄の数が数秒「…」と古い数のまま。ito に頼む。
- 読み取りが追いついていない間、選択の文字数は「…」なのに、文書の文字数は古い数のまま印が無い（TESTING 8.12 では設計どおりとしている）。
