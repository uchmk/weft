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
- [ ] ito に頼んだもの（入れば weft の回り道を外す）:
  - uchmk/ito#159 `ito-textbuf`: 行の索引（今の `pos` の行の関数は O(n)）、元に戻すで保存した所に戻ったら未保存でなくなる印、続けて消した文字を 1 歩にまとめる。
  - uchmk/ito#160 `ito-mdedit`: リストの項目 1 つだけを上下に動かす（今の `move_block` はリスト全体を動かす）。
  - uchmk/ito#161 `ito-outline`: `at_byte` が本文の末尾のカーソルを取りこぼす（weft は 1 バイト前で聞いて回避）。

### フェーズ 2 日本語特化
- [ ] `ito-ime` の統合。確定の前後でカーソルと選択を保つ。
  変換中の文字をカーソルの所に出す・確定を入れる・候補の窓をカーソルに寄せる・変換中のキーをキーマップに渡さない、までは `src/view.rs` で配線済み。
- [ ] 入力の優先順位（IME → 補完 → キーマップ → コマンド）。変換中は補完を開かない、確定の Enter をコマンドにしない。
- [ ] 全角スペースと意図しないハード改行を控えめにハイライト。
- [ ] 状態欄: 行・列、日本語の文字数、選択の文字数、保存状態、文字コード、改行コード、IME の状態（`ito-statusbar`）。

### フェーズ 3 Markdown 操作（`ito-mdedit` を配線する）
- [x] `block.move_up` / `block.move_down`（`move_block`、`Alt+↑` / `Alt+↓`）、節の移動（`move_section`、`Alt+Shift+↑` / `Alt+Shift+↓`）。リストの項目 1 つは uchmk/ito#160 待ち。
- [x] `section.promote` / `section.demote`（`promote_section` / `demote_section`。子孫の見出しごと。`Alt+Shift+←` / `Alt+Shift+→`）。
- [x] リストの階層（`indent_item` / `outdent_item`、`Alt+→` / `Alt+←`）、`list.toggle_task`（`toggle_task`、`Ctrl+Enter`）、番号の振り直し（`renumber`、パレットから）。
- [ ] `table.format`（`format_table`。全角は 2 セル幅）、`table.next_cell`（`next_cell`、Tab / Shift+Tab）。
  コマンドとパレットには入れた。Tab / Shift+Tab は表の中だけで効くように割り当てる所が残っている（表の外の Tab をどうするかと一緒に決める）。
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
