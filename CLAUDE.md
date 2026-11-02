# weft — Claude 向けメモ

Markdown 専用エディター。キーだけで見出し・リスト・表などの構造を組み替え、日本語 IME に強く、フォルダー（vault）も扱う。kura の `.md` の opener。
uchmk のアプリで、共通の部品は [ito](https://github.com/uchmk/ito) にある。Rust、Windows 優先。

## 設計（判断に迷ったら docs/design.md へ戻る）

設計の全体は [docs/design.md](docs/design.md)。作業の前に読む。特に守ること:

- **7 つの原則**: (1) Markdown が主役。汎用テキストエディターにしない。(2) キーボードだけで完結する。(3) 操作はすべて名前つきコマンド。
  (4) 日本語 IME を最優先。(5) 編集操作は構造を理解する。(6) 入力を絶対に待たせない。(7) ito・kura の資産を最大限再利用する。
- **作らないもの**: マクロ・拡張機能・プラグイン・Vim 風のモード・レジスタ・スクリプト言語・マルチカーソル・矩形選択・複雑な WYSIWYG。汎用の編集は `$EDITOR` へ（ito-openin）。
- **コマンド**: UI に依存しない。キー・パレット・ツールバー・テスト・`--keys` から同じコマンドを呼ぶ。名前は「対象.動作」
  （`block.move_up`・`section.promote`・`list.toggle_task`・`table.format`・`table.next_cell`・`heading.fold`・`heading.insert_toc`）。
  ツールバーとボタンはコマンドの一覧（ito-palette と同じ元）から作り、アイコン＋文字にする。アイコンが ito に無ければ ito に頼む。
- **層**: UI ／ コマンド ／ **Document Engine**（バッファ・カーソル・選択・元に戻す・構造木・ソース範囲。UI を持たず単体でテストする）／ ファイル ／ vault。
- **トランザクション**: 1 回の操作は 1 回の元に戻すで完全に戻る。ファイルへの作用（画像の保存・リネームなど）も逆操作を持たせて同じトランザクションに入れる。
- **位置**: 構造解析はバイト範囲、カーソルは文字境界（書記素クラスタも考慮）。変換は Document Engine の位置モジュール 1 か所だけで行う。
- **入力の優先順位**: IME の変換中なら IME → 補完ポップアップ → キーマップ → コマンド。変換中の Enter は確定であってコマンドではない。
- **入力の経路**: 入力から画面更新までに Markdown 全文の再解析を入れない。パース・色づけ・差分・索引はワーカーで版番号つきに行い、古い版の結果は捨てる。
- **保存**: 改行コード（CRLF・LF・混在）と BOM は読んだまま書き戻す。一時ファイルに書いてから置き換える。

## このアプリで使う ito のクレート（自前で書かない）

| 役目 | クレート |
|---|---|
| 構造コマンド（ブロック・節の移動、昇格・降格、リスト、タスク、番号、表） | `ito-mdedit`（`(text, caret_byte) -> Option<Change>`、`Change::apply`） |
| 見出しの一覧・目次の列 | `ito-outline` |
| 整形プレビュー（原文のバイト ⇔ 整形行） | `ito-mdview` |
| ミニマップ（原文側） | `ito-minimap` |
| 色づけ（`Stream` は `Clone`。大きなファイルは N 行ごとの保存点） | `ito-syntax` |
| 日本語入力 | `ito-ime` |
| あいまい検索・`[[…]]` の絞り込み | `ito-match`・`ito-palette` |
| 差分（ガター） | `ito-diff` |
| 全文検索の索引 | `ito-store` |
| 外部変更の検知 | `ito-watch` |
| `$EDITOR`・既定のアプリで開く | `ito-openin` |
| 書き出し・SVG→画像 | `ito-export` |

ここに無い役目は、ito の `CLAUDE.md` の表を見てから。無ければ下の「ito first」のとおり ito に頼む。

## ito first（最優先の約束）

**ito にあるものは ito を使い、ないものは自分で書かずに ito に頼む。**

- 新しい機能を足す前に、ito の `CLAUDE.md` の表（`C:\dev\ito\CLAUDE.md`）で、同じ役目のクレートがないか見る。
- ないときは、uchmk/ito に Issue を立てる（雛形 `crate-request`）か、ito のセッションに頼む。
  このアプリに先に書いて、あとで ito に移すことはしない（移す手間が残り、二重になる）。
- `scripts/ito-first.py` が機械で見る。ito が受け持つ依存（`toml_edit`・`dirs`・`notify` …）を直接足すと落ちる。
  持つべき ito のクレート（keyscript・help・notify・bugreport …）が抜けていても落ちる。
  理由があって外すときだけ `scripts/ito-first.allow` に `<crate> <理由>` を書く（理由のない行は効かない）。
- 全部の ito のクレートは 1 つの `rev` にそろえる。上げたら `scripts/lanes.sh sync`。
  `.github/workflows/` の build・arm64・checklists・release・release-sums は ito の雛形から `sync` が書く（`lanes.conf` の `workflows=`）。直すなら ito で。

## 画面は早見表から

画面の部品（ボタン・アイコン・空の一覧・確認・パレット・Markdown の欄など）を作る前に、ito の `docs/design-guide.md`（`C:\dev\ito\docs\design-guide.md`）で、使えるクレートを探す。

## 状態のファイルは TOML

アプリが機械的に書いて読み戻すもの（窓の場所・履歴・最近開いたもの・セッション）は `ito_common::state_dir("weft")` に **TOML** で置く
（`window.toml`、`state.toml`、`recent.toml` …）。`.txt`・`.json`・拡張子なしのバイナリで持たない。

- 一覧は表の中の配列（`recent = ["…"]`、`[[entries]]`）。読むのは `ito_common::kept::read`、書くのは `kept::Writer`（別スレッド・最新が勝つ・`--keys` では書かない）。UI スレッドで書かない。
- ログは `.log`。キャッシュ・画像・WebView のデータなど、読み戻さないものは対象外。秘密の鍵は `app_dir` の中で、TOML でなくてよい。
- 詳しくは ito の `docs/common-spec.md` の「設定の形式は TOML」。

## ボタンは ito-prefs のものを使う

画面のボタンを `egui::Button` で自前に作らない。大きさ・角・フォーカスの輪・テーマの色が画面ごとに変わってしまう。

- ふつうのボタンは `ito_prefs::button(ui, look, "文言") -> bool`。
- 進める側（主な操作）や、取り返しのつかない操作は `ito_prefs::button_with(ui, look, "文言", Tone::Primary / Tone::Danger) -> Response`（テーマの強調色・エラー色で塗る）。やめるなど静かなものは `Tone::Plain` のまま。
- 区切りのボタンは `ito_prefs::seg`、確認の箱のボタンは `ito-confirm`（`Question::primary`・`danger`・`keys`）。
- 一つの行に主なボタンは一つだけ。Esc／Enter が押すものを主にする。

## 空の一覧は `empty_state`

一覧・表・パネルが空のときに、自前で「ありません」とだけ書かない。`ito_prefs::empty_state(ui, look, mark, "何が空か", "何をすればよいか", action) -> Option<Response>` で、
薄い絵＋文言（ゴースト）を出す。

- 出す条件は呼ぶ側が決める（`if items.is_empty()` のとき、一覧の代わりに）。
- 追加できる空（まだ 1 件もない）は `action: Some("追加")` で、下に主なボタンを 1 つ出す。押されたかは返り値。
- 追加できない空（絞り込みで 0 件・読み取り専用・権限なし）は `None`。ボタンは出さず、文言で理由を言う。
- 文言は `lang/*.toml` のキーで持つ（日英）。

## 作業ルール

- 返答は日本語。コード・コメント・コミットメッセージは英語。
- 改行は LF。
- コミットと push は、`scripts/verify.sh` が `ALL OK` のときに、頼まれなくてもしてよい（`main` へ直接。force push・履歴の書き換えはしない）。自分の変更だけを `git add` する。
- 整形は `cargo fmt`（ito と同じ `rustfmt.toml`）。clippy は `-D warnings`。
  全体を整形しただけのコミットは `.git-blame-ignore-revs` に書く（`git config blame.ignoreRevsFile .git-blame-ignore-revs`）。
- ビルドは R:（RAM ディスク）。新しいクローンでは `pwsh -NoProfile -File %LOCALAPPDATA%\uchmk-build\cargo-ramdisk.ps1` を先に回す。
- やることは [TODO.md](TODO.md)。版の繰り上げと CHANGELOG.md（日本語、日付は JST）はセットでコミットする。
- 共通の仕様（common.toml・設定の画面・テーマ・時計）は ito の `docs/common-spec.md`。
