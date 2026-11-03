# TODO

雛形（ito の `templates/app`）から作った。雛形の配線は、窓（場所を覚える）、common.toml のテーマ・言語・倍率、
`--version` / `--help`、F12 の不具合報告、`weft.com`、アイコンと版の情報、`--keys`、keymap.toml、F1 のヘルプ、設定の画面、パレット、字体まで済んだ。
下の `[ ]` は、編集の画面・保存ができてから配線するもの。自前で書かずに ito のクレートを使う
（足りない所は ito に頼む。`scripts/ito-first.py` が見ている）。

- [x] `--keys` の台本: `ito-keyscript` の `Runner`（`State`・`Shot`・`Theme`・`Lang`・`Read`・`Quit`）。`WEFT_KEYS_DONE` は `lanes.conf` の `keys_env` と同じ。
  状態のファイルは `screen`（main・settings・help）・`lang`・`page`・`palette`・`notices`。
- [ ] `--keys` の残り: 状態・木・アナウンス（`<Enabled:>`・`<Focused:>`・`<Tree:名前>`・`<Announced:>` など）は、`ito-a11y` を入れていないので未配線。
  アナウンスは `ito_a11y::announce` を使い、`ito-a11y` の `egui` の機能を入れる。通知は `Cargo.toml` の機能 `test-hooks`（`ito-notify/test-sink`）を入れたビルドで
  `ITO_NOTIFY_SINK=<file>` に記録できる。出荷するビルドには入れない。編集の画面ができてから入れる。
- [x] `keymap.toml` と設定の「キー」ページ: `ito-keymap`・`ito-prefs` の `keys_page`。既定のキーは `defaults/keymap.toml`
  （`<C-,>` 設定・`<F1>` ヘルプ・`<C-q>` 終了）。keymap.toml には変えた分だけを書く。読めない行はベルに出す。
- [x] F1 のヘルプ: `ito-help`。`?` は文字の入力に使うので割り当てない（設計の原則）。
- [x] 通知と設定の「通知」ページ: ページ（`notify_page`、試しの通知）と、保存できなかったときの `notify::tell`（`take_files`）。
- [x] 設定の画面: `ito_prefs::show`（一般・テーマ・キー・通知・詳細・情報）。`Ctrl+,` で開き、`Esc` で閉じる。
- [x] 窓の場所を戻す: `window::Saved` で戻し、`Tracker::follow` で書き、`on_exit` で `Tracker::close`。
- [ ] 通知を入れる: 設定・keymap.toml・テーマ・言語の読めなかったことはベルに `Kind::Warn` で入れる（`go` は `()`）。
  `ito-notify` の `Toast::open` のトーストは、飛び先（開いているファイル）ができてから。
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
  IME のオン・オフと入力モードを読む手段が `ito-ime` に無いので uchmk/ito#166 で頼んだ（今は「変換中」だけ出せる）。

### フェーズ 3 Markdown 操作（`ito-mdedit` を配線する）
- [x] `block.move_up` / `block.move_down`（リストの項目は `move_item` で子の項目ごと 1 つ、それ以外は `move_block`。`Alt+↑` / `Alt+↓`）、節の移動（`move_section`、`Alt+Shift+↑` / `Alt+Shift+↓`）。
- [x] `section.promote` / `section.demote`（`promote_section` / `demote_section`。子孫の見出しごと。`Alt+Shift+←` / `Alt+Shift+→`）。
- [x] リストの階層（`indent_item` / `outdent_item`、`Alt+→` / `Alt+←`）、`list.toggle_task`（`toggle_task`、`Ctrl+Enter`）、番号の振り直し（`renumber`、パレットから）。
- [ ] `table.format`（`format_table`。全角は 2 セル幅）、`table.next_cell`（`next_cell`、Tab / Shift+Tab）。
  コマンドとパレットには入れた。Tab / Shift+Tab の割り当てが残っている。決めたこと（2026-11-03）: 表の中はセルの移動、リストの項目では字下げ・戻し（`indent_item` / `outdent_item`）、
  それ以外では Tab が空白を入れ、Shift+Tab が行頭の空白を戻す。空白の数は設定で選べる（既定は 4）。
- [ ] 見出しの折りたたみ（`heading.fold`）、目次の挿入・更新（`heading.insert_toc`）。
  折りたたみは `ito-outline` の `section` を使う weft の表示。目次の挿入・更新は uchmk/ito#163 で頼んだ。
- [ ] 構造選択（単語 → 段落 → リスト → 節 → 親の節）、構造を壊さない削除、構造ナビゲーション、構造診断（lint）、集中表示。
  構造選択・削除・ナビゲーションは uchmk/ito#163、lint は uchmk/ito#164 で頼んだ。集中表示は `ito-outline` の `section` を使う weft の表示。

### フェーズ 4 実用エディター
- [ ] 自動保存・強制終了からの復元、外部変更の検知（`ito-watch`。未保存があれば差分を見せて選ばせる）。
- [ ] 画像の貼り付け（添付フォルダー `assets/` へ WebP、日時＋連番、ハッシュで重複を再利用、1 回の元に戻すでファイルもゴミ箱へ）。
  非可逆 WebP の手段は ito・kura に無かった（kura は `image` で読むだけ、`image` の書き出しは可逆のみ。クリップボードの画像を読む部品も無い）。
  uchmk/ito#162 で頼んだ（クリップボードの画像を読む・非可逆 WebP に書く）。入るまで待つ。
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
  weft の側（`+行` / `+行:列` を受け付ける）は作業中。kura の `.md` の opener を weft にするのは kura の側。

### その後（設計の「後回し」）
- 数式（v2）・Mermaid（v2〜v3。ライブラリの選定は ito への crate-request の候補）、書き出し、動画、テンプレート、タスク・タグの一覧、
  日本語の整形ルール、見出しのナンバリング、ライブプレビューを作るかの判断。

## 見つけた課題（ユーザーが認めるまで手をつけない）
- 編集欄の等幅フォントで、全角の括弧・数字・英字（U+FF01〜FF5A）が egui の `emoji-icon-font` の字形で描かれる（日本語の書体を族の最後に足しているため）。
  全角スペース（U+3000）はメイリオから全角で出ている。また JetBrains Mono（0.6 em）に対し漢字は 1.0 em で 1.67 桁になり、`format_table` で整えた表が編集欄で揃わない。
  字の並びを ito で一か所にまとめるのを uchmk/ito#167 で頼んだ（7 つのアプリが同じ処理を書いている）。入ったら `take_font` をそれに置き換える。
- 文字数の桁区切り（`12,345`）の部品が ito-common に無いので uchmk/ito#168 で頼んだ（minato・kura に同じものがある）。状態欄は今は区切らずに出している。
- 範囲の書記素を数える部品が ito-textbuf の `pos` に無いので uchmk/ito#169 で頼んだ。入ったら weft の `count::count` をそれに置き換える。
