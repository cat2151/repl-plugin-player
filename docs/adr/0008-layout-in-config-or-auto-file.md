# ADR 0008: user の配置は `config.toml` の `[layout]` に、手で動かした配置は `layout-auto.toml` に置き、どちらを使うかを設定の menu で切り替える

- 状態: 採用
- 決定者: user（console の最初の配置はフローティングにせず右下の tile にすること・動かした配置は tile の並びを含めて全部覚えること・tile の場所を含む配置をすべて toml に書いてホットリロードすること・user が書いた配置をアプリに書き換えさせないこと・user が toml で指定した配置と手で動かして自動保存される配置を区別して切り替えられること・user の配置を `config.toml` に入れること・切り替えを `Ctrl+,` の menu で行い `config.toml` には書かないこと・dock window を `../cat-smf-editor` のようにモニタに対する割合で置けること・2つのモードを混ぜず、切り替えた瞬間に本体と dock window の座標と大きさをまとめて切り替え先のものにすること）。切り替えを `status.json` に保存すること・既定を自動保存にすること・自動保存でないときは何も書かないこと・切り替えたときにその場で読み直すこと・形式を `../cat-smf-editor` の `layout.toml` に揃えること・`repl.ui` の窓を書かないことは AI の提案
- 関連: [0006](0006-dock-with-egui-tiles.md)（tile と OS window の構成。「配置は保存しない」をこの ADR で置き換える）

## 決定

- user の配置は `config.toml` の `[layout]` に書く。アプリは `config.toml` を読むだけで書かない。手で動かした配置は `%LOCALAPPDATA%\repl-plugin-player\layout-auto.toml` にアプリが書く。
- どちらを使うかは `Ctrl+,` の menu の「配置」で「手で動かした配置」か「config.toml に書いた配置」かを選び、`status.json` の `layout_auto` に残す（無ければ自動保存）。切り替えると、自動保存の側は今の配置を書いてから、もう一方を読み直して反映する。
- 本体ウィンドウの位置と大きさも配置に含める。自動保存では `layout-auto.toml` の `[main]`（`x`・`y`・内寸 `width`・`height`）に、そうでないときは `config.toml` の `[window]` に置き、起動時も使っている側から取る。
- 切り替えた瞬間に、本体ウィンドウを切り替え先の位置と大きさへ動かし、dock window を全部閉じて切り替え先の構成で開き直す。前のモードの位置は1つも残さない。
- 自動保存のときだけ、配置が変わると 500ms ごとと終了時に `layout-auto.toml` へ書き出す。自動保存でないときは、手で動かしても何も書かない。
- 使っている側のファイルの中身が最後に読んだ／書いた文と違えば読み直して反映する。`config.toml` に `[layout]` が無ければ最初の配置にする。
- 形式は両方同じ。`root` に tree を `row`・`column`・`tabs`・`grid`・`view`（`width`・`height` は比率、tabs の子の `active = true`）で書き、dock window は `window` の配列に、同じ形の tree と外枠の位置 `x`・`y`・内寸 `width`・`height` で書く。代わりに `[window.monitor_placement]`（`monitor` は `"left"`・`"center"`・`"right"` か DISPLAY 番号、`left_percent`・`top_percent`・`width_percent`・`height_percent` はそのモニタの作業領域に対する外寸の百分率）でも書ける。両方は書けない。`config.toml` では `[layout.root]`・`[[layout.window]]`・`[layout.window.monitor_placement]` になる。
- `monitor_placement` は配置を読んだときにモニタへ当てはめ、window が開いてから数 frame の間だけ物理 pixel で動かす。自動保存でも `monitor_placement` はそのまま書き出し、手で動かした位置では上書きしない。
- 読めない配置（toml の誤り・無い view・同じ view が2か所・不明な key・つながっていないモニタ・位置が1つに決まらないモニタ）は反映せず、直前の配置のまま main の欄にファイル名つきの赤字で出し、自動保存でも直るまで書き出さない。書かれなかった既定の欄は active なタブの列へ足す。
- console は独立した欄にし、最初の配置では右の列の一番下の tile にする。
- `repl.ui` の窓は書かない。

## 理由

- user の要件: 動かした結果を覚えつつ、配置を toml で書いて即反映したい。ただし user が書いた配置をアプリが書き換えるのは致命的。1つのファイルをアプリと user の両方が書くと、user の配置とコメントが手で動かした結果で上書きされる。書き手ごとにファイルを分ければ、user の配置は user だけが変える。
- 切り替えは menu で操作する状態で、アプリが書く。user だけが書く `config.toml` には置けないので、external editor の登録と同じ `status.json` に置く。`status.json` は key ごとに書き、他の key を消さない。
- 自動保存でないときに `layout-auto.toml` へ書かないのは、自動保存へ戻したときに「最後に自動保存で使っていた配置」が残るようにするため。
- 形式は user が既に使う `../cat-smf-editor` の `layout.toml` に揃え、書き方を覚え直さずに済むようにした。`layout-auto.toml` の中身を `[layout]` の下へ写せば user の配置にできる。
- `repl.ui` の窓は cell を実行するまで registry に無く、起動時に復元できない。
- 切り替えで位置を1つでも持ち越すと、どちらのモードの配置なのか user が分からなくなる（user の判断）。dock window を開き直すのは、切り替え先に位置の無い window が前のモードの位置に残り、それが自動保存に書かれて混ざるため。
- `monitor_placement` を egui の `with_position`・`with_inner_size` でなく Win32 の `SetWindowPos` で物理 pixel に置くのは、DPI の違うモニタをまたぐと論理座標の換算がずれるため（`../cat-smf-editor` と同じ方式）。

## 却下した案

- **1つの配置ファイルをアプリが書き戻し、user も編集する。** user が書いた配置とコメントが、手で動かすたびに失われる。
- **user の配置を `config.toml` と別の `layout.toml` に置く。** user の要件（`config.toml` に統合）に合わない。
- **切り替えを `config.toml` に書く。** user の要件（menu で切り替える）に合わない。
- **eframe の persistence（`persist_window` と storage）。** user が読んで書ける toml にならず、ホットリロードもできない。
- **自動保存でないときも手で動かした配置を `layout-auto.toml` へ書く。** 自動保存へ戻したとき、自動保存で作った配置が上書きされている。

## 結果

- 自動保存のとき、user が `layout-auto.toml` を書いている途中にアプリが書き出すと、user の編集が上書きされうる（書き出しの直前に読み直すので、間は最長 500ms の poll 1回分）。user が残したい配置は `config.toml` に書く。
- 自動保存でないとき、`config.toml` の `[layout]` 以外の key を書き換えても、配置は `[layout]` の内容へ戻る（ファイルの文が変わると読み直すため）。
- 本体ウィンドウに位置の無い `config.toml` へ切り替えると、本体は大きさだけ変わり、位置は切り替える前のまま残る（OS 任せの位置は作り直せない）。
- `monitor_placement` はファイルを読んだときのモニタ構成で決まる。起動後にモニタをつなぎ替えても、ファイルの文が変わるまで置き直さない。Windows 以外では読めない配置として扱う。
