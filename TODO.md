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
- [ ] 通知と設定の「通知」ページ: ページ（`notify_page`、試しの通知）は配線済み。`notify::tell`（保存できなかった知らせ）は、保存ができるまで呼ぶ所がない。
- [x] 設定の画面: `ito_prefs::show`（一般・テーマ・キー・通知・詳細・情報）。`Ctrl+,` で開き、`Esc` で閉じる。
- [x] 窓の場所を戻す: `window::Saved` で戻し、`Tracker::follow` で書き、`on_exit` で `Tracker::close`。
- [ ] 通知を入れる: 設定・keymap.toml・テーマ・言語の読めなかったことはベルに `Kind::Warn` で入れる（`go` は `()`）。
  `ito-notify` の `Toast::open` のトーストは、飛び先（開いているファイル）ができてから。
- [x] コマンドパレットの中身: 不具合の報告・拡大率を戻す・設定・キーのヘルプ・終了。キーは `ito-keymap` から引く。編集のコマンドはフェーズ 1 で足す。
- [x] 字体: `ito-fonts` で日本語の字体を別スレッドで読み、`egui` に足す。
- [x] `TESTING.md` に確認の行（起動・キーのヘルプ・設定・パレット・通知・拡大率・台本）を足し、`cargo run --example make-testcheck` で `TESTING-CHECKS.md` を作った。
- [x] `icon.rs` に本当のアイコン（織り目。縦糸と横糸が交互に上下する）。`.exe` のアイコンも同じ絵から `build.rs` で作る。
- [x] リリースの workflow: `scripts/lanes.sh sync` が ito の雛形から書く（`lanes.conf` の `workflows=`）。

## weft の機能（フェーズ順。設計は [docs/design.md](docs/design.md)）

フェーズ 1〜3 が済めば、既存のエディターと違う価値を持つ。ここまでを最初の目標にする。
ワーカーへの分離（版番号つきの依頼、古い結果を捨てる）は、フェーズ 1 から設計に入れる。

### 着手前（設計の「進め方」の (3)・(4)）
- [x] 編集バッファ（rope など）と元に戻すを、ito・kura が既に持っていないか調べる → どちらにも無い（kura の `UndoStep` はファイル操作の元に戻す）。
- [ ] 編集バッファ・位置・選択・トランザクションの元に戻すを ito に頼んだ（uchmk/ito#156）。`ito-textbuf` として ito の 0126b34（v0.222.0）に入った。
  weft は既にこの rev。フェーズ 1 で `ito-textbuf` を使い、weft に先に独自の型を作らない。
- [ ] コマンドエンジンの設計: 「対象.動作」の名前、ito-keymap の割り当て、ito-palette の一覧、ツールバーを 1 つの元から出す。コマンドの列を 1 つの名前にまとめる所まで（スクリプト言語は持たない）。

### フェーズ 1 編集エンジン（Document Engine）
- [ ] バッファ・カーソル・選択（文字境界、書記素クラスタ）。
- [ ] 位置モジュール（バイト範囲 ⇔ 文字境界の変換を 1 か所に）。CRLF・全角・絵文字のテスト。
- [ ] 元に戻す・やり直す（1 操作 = 1 トランザクション。ファイルへの作用の逆操作も持てる形）。
- [ ] 構造木（pulldown-cmark の `into_offset_iter`、kura の `src/preview/markdown.rs` と同じ options）。ワーカーで版番号つきに作る。
- [ ] `ito-mdedit` の `Change` をトランザクションとして適用する所。
- [ ] 名前つきコマンドとキーマップ（`ito-keys`・`ito-keymap`）、パレット。
- [ ] 色づけ（`ito-syntax`）をワーカーで。追いつくまでは前の色のまま文字だけ出す。
- [ ] 改行コード・BOM を読んだまま保存（一時ファイル → 置き換え）。UTF-8 以外は警告して読み取り専用か変換。
- [ ] 1 キーで `$EDITOR` へ（`ito-openin`、行・列を渡す。kura の `[line_args]` に揃える）。

### フェーズ 2 日本語特化
- [ ] `ito-ime` の統合。確定の前後でカーソルと選択を保つ。
- [ ] 入力の優先順位（IME → 補完 → キーマップ → コマンド）。変換中は補完を開かない、確定の Enter をコマンドにしない。
- [ ] 全角スペースと意図しないハード改行を控えめにハイライト。
- [ ] 状態欄: 行・列、日本語の文字数、選択の文字数、保存状態、文字コード、改行コード、IME の状態（`ito-statusbar`）。

### フェーズ 3 Markdown 操作（`ito-mdedit` を配線する）
- [ ] `block.move_up` / `block.move_down`（`move_block`）、節の移動（`move_section`）。
- [ ] `section.promote` / `section.demote`（`promote_section` / `demote_section`。子孫の見出しごと）。
- [ ] リストの階層（`indent_item` / `outdent_item`）、`list.toggle_task`（`toggle_task`）、番号の振り直し（`renumber`）。
- [ ] `table.format`（`format_table`。全角は 2 セル幅）、`table.next_cell`（`next_cell`、Tab / Shift+Tab）。
- [ ] 見出しの折りたたみ（`heading.fold`）、目次の挿入・更新（`heading.insert_toc`）。
- [ ] 構造選択（単語 → 段落 → リスト → 節 → 親の節）、構造を壊さない削除、構造ナビゲーション、構造診断（lint）、集中表示。
  `ito-mdedit` に無いので、要る形が決まったら ito に頼む（アプリに書かない）。

### フェーズ 4 実用エディター
- [ ] 自動保存・強制終了からの復元、外部変更の検知（`ito-watch`。未保存があれば差分を見せて選ばせる）。
- [ ] 画像の貼り付け（添付フォルダー `assets/` へ WebP、日時＋連番、ハッシュで重複を再利用、1 回の元に戻すでファイルもゴミ箱へ）。
  非可逆 WebP の手段は ito・kura を調べてから。無ければ ito に頼む。
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

### その後（設計の「後回し」）
- 数式（v2）・Mermaid（v2〜v3。ライブラリの選定は ito への crate-request の候補）、書き出し、動画、テンプレート、タスク・タグの一覧、
  日本語の整形ルール、見出しのナンバリング、ライブプレビューを作るかの判断。
