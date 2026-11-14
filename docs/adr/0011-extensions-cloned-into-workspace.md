# ADR 0011: 拡張 DLL は別 repo でも `extensions/<名前>/` に置いて同じ workspace で build し、設定の画面で on にする

- 状態: 採用
- 決定者: user（拡張の DLL をいつでも別 repo にできる形にし、keyboard も「たまたま内蔵しているだけ」の1つとして同じ扱いにすること・別 repo はこの repo の `extensions/<名前>/` に置いて同じ cargo workspace で build し、版の照合や C ABI を採らないこと・本体から DLL へ渡す型を workspace の中の型だけの共通 crate で共有すること・別 repo を置くとこの repo の `Cargo.lock` が変わるのを許容すること・on/off を layout と切り離して設定で決めること・拡張の repo を兄弟に置いて junction で指し、この repo で test を通してから拡張の repo だけを commit・push する開発の流れ・1つの DLL に複数の拡張を入れ、拡張を1つずつ選ぶこと・タブの題を DLL 名を除いた拡張名にすること・内蔵の DLL を `builtin` と名付けること・on/off を設定の画面の checkbox で選び、保存先を `config.toml` から json へ移すこと・画面を `Ctrl+,` の設定の中に置くこと・画面を開いたときに候補の DLL を全部読んで拡張名を並べること・「反映は再起動後」を起動時から変えたときだけ出すこと・どの DLL にも無い on の名前も並べて off にできること）。名前の形・保存先を `status.json` の `extensions` にすること・無効な拡張の view を黙って捨てること・共通 crate の場所と中身・文言を DLL が持つこと・`extensions/` の下を全部読んで描く test は AI の提案
- 関連: [0007](0007-gui-extensions-as-dll.md)（DLL を同じ workspace で build して `egui::Ui` を直接渡すこと。この ADR はその一部を置き換える）、[0006](0006-dock-with-egui-tiles.md)（欄）、[0008](0008-layout-in-config-or-auto-file.md)（layout の置き場所）、[0005](0005-locale-overlay.md)（locale の上書き）

## 決定

- workspace は `members = [".", "extensions/*", "extension-api"]`、`default-members = [".", "extensions/*"]` の glob にする。別 repo の拡張は `extensions/<DLL名>/` に置けば、引数なしの `cargo build` で本体と同じ回に build される。`.gitignore` は `extensions/builtin` 以外の `extensions/*` を追わない。
- 本体から DLL へ渡す型は root の `extension-api/`（package `repl-extension-api`）だけに置く。中身は `Frame`（鳴っている音・音域・本体の言語）、`Show`（`fn(&mut egui::Ui, &Frame)`）、`Extension`（拡張名と `Show`）、DLL が export する一覧の関数 `List`（`fn() -> &'static [Extension]`）とその symbol 名 `repl_extensions`、egui の一致を確かめる `repl_egui_type_id` だけ。拡張でないので `extensions/*` の glob から外す。
- 1つの DLL は拡張を1つ以上持つ。同じ DLL の拡張どうしは DLL の static を共有できる。
- 拡張の名前は `<DLL名>/<拡張名>` で、`config.toml` の `name` ＝ layout の `view` 名。DLL 名 ＝ package 名 ＝ DLL の stem ＝ `extensions/` の dir 名。タブの題は拡張名だけ。この repo の DLL は `builtin`（`builtin.dll`）で、keyboard は `builtin/keyboard`。
- `status.json` の `extensions`（`"<DLL名>/<拡張名>"` の配列）にある拡張だけを on にし、無ければ off（keyboard も同じ）。同じ DLL の拡張も1つずつ持ち、DLL 単位の一括指定や `/` の省略形は持たない。`/` が1つでない名前はエラーにして読まない。読むのは起動時だけ。`extensions` が無いときの既定は `builtin/keyboard` を on にする。`config.toml` には書かない。
- on/off は `Ctrl+,` の設定の中の checkbox で選び、切り替えた瞬間に `status.json` へ書く。設定を初めて開いたときに `extensions/` の dir 名（repo の外では埋め込んだ物）の DLL を全部読み、その拡張を並べる。読めない DLL は理由を赤字で出す。どの DLL にも無い on の名前は赤字の行で並べる。起動時と違う組を選んでいる間だけ「反映は再起動後」を出す。
- DLL は on の拡張が1つでもあるときだけ、その欄を最初に描くときに読む。DLL に無い拡張名は、欄に DLL にある拡張名を赤字で出す。
- 欄は `Pane::Extension(name)`。on の拡張は layout が省いていれば自動で足す。off の拡張の `view`（`extensions/` にある DLL 名で始まる物）は layout にあっても黙って捨て、空になった container も捨てる。それ以外の、拡張でも固定の欄でもない名前は `layout_view` エラー。
- DLL の場所は 0007 と同じ規則で、`src/resources.rs` の `extension(name)` が決める。repo の外の exe は埋め込んだ拡張だけを読み、埋め込みに無い名前は赤字にする。埋め込むのは repo に入っている拡張だけで、一覧は `scripts/build.py` の `BUNDLED`。
- keyboard の tooltip の文言は DLL が ja/en で持ち、`Frame.language` で選ぶ。
- 本体の test は `extensions/` の下の DLL を全部読み、その拡張を1つずつ1回描く（読めない・描けないなら失敗し、skip しない）。

## 理由

- 同じ workspace・同じ `Cargo.lock` で build すれば、0007 の「`egui::Ui` を直接渡せる」前提がそのまま保たれ、共通 crate の型も本体と DLL で同じ物になる。
- user の要件「いつでも別 repo にできる」「keyboard はたまたま内蔵」。内蔵と別 repo を同じ設定・同じ欄・同じ読込で扱えば、keyboard を外へ出すときにコードが変わらない。
- 1つの DLL に複数の拡張を入れるのは、欄どうしで状態を共有するため（DLL が分かれると static を共有できず、本体を経由する口が要る）。DLL ごとに egui を link する大きさと時間、junction と `Cargo.toml` の数も減る。
- 名前を `<DLL名>/<拡張名>` にすると、config から読む DLL が決まり、全 DLL を読んで一覧を作らずに済む。違う DLL に同じ拡張名があっても `view` 名はぶつからない。
- 一覧を export するので、拡張名を間違えたときに、DLL にある名前を欄に出せる。
- off の拡張の view を捨てるのは、off にした瞬間に `config.toml` や `layout-auto.toml` の layout 全体が読めなくなるのを防ぐため。
- 保存先を `status.json` にするのは、画面でアプリが書くからで、`config.toml` はアプリが書かない（[0008](0008-layout-in-config-or-auto-file.md)）。
- 画面で DLL を全部読むのは、DLL の中の拡張名が読まないと分からないため。読むのは `extensions/` に置いて自分で build した物か埋め込んだ物だけで、却下した「フォルダ内の DLL を全部読む」とは違い、置かれた任意のファイルは読まない（user の判断）。
- どの DLL にも無い名前を並べるのは、並べないと `status.json` を手で直すしか off にする手段がないため。
- 別 repo の DLL は本体の locale を知らないので、文言は DLL が持つ。
- 拡張の package は test を持たない（0007）ので、拡張の repo の作者が「この repo で test を通す」ときに自分の DLL まで読んで描く test が本体側に要る。

## 却下した案

- **版の照合つきの Rust ABI。** 照合では egui の feature の違いを見抜けない。
- **C ABI。** egui の widget が使えず、keyboard を書き直すことになる。
- **std と egui の型だけで口を固定する。** 渡す物を増やすたびに全 DLL の口が変わる。
- **フォルダ内の DLL を全部読む。** 置かれた物を何でも実行することになる。
- **DLL 単位で on/off し、画面では DLL を読まない。** 拡張を1つずつ選べない。
- **`config.toml` の `[[extension]]` を画面からも書く。** user が書いた `config.toml` とコメントをアプリが書き換える（0008）。
- **1 DLL = 1 拡張のまま。** 欄どうしで状態を共有できない。
- **DLL を1つの欄にし、中のタブで拡張を切り替える。** 2つの拡張を同時に並べて dock で配置できない。
- **拡張ごとに symbol を export する（`repl_extension_show_<拡張名>`）。** DLL にある拡張名を本体が知れず、名前を間違えたときに候補を出せない。
- **layout に書かなければ off。** layout を読まないときに既定の配置へ戻り、黙って on になる（user が却下）。

## 結果

- 別 repo を `extensions/` に置くと、この repo の `Cargo.lock` に依存が足され、`--locked` の build が落ちる。外すと引数なしの `cargo build` で戻る。
- `extensions/` に `Cargo.toml` の無い dir があると、cargo がその dir 名を出して止まる。
- 拡張の repo は単独では build できない（`failed to find a workspace root`）。その `Cargo.toml` の `path` は junction 側の位置（`extensions/<名前>/`）から数える。
- junction を中身ごと消す削除で外すと、拡張の repo の中身まで消えるおそれがある。外すときは `rmdir` を使う。
- junction を外しても `target/release/<名前>.dll` は残るが、本体が読む拡張の名前は `extensions/` の dir と `status.json` の `extensions` から決まるので読まれない。
- 設定を一度開くと、off の拡張の DLL も読み込まれ、終了まで載ったままになる（keyboard の DLL で約 3.9 MB）。DLL の初期化の処理もこのとき走る。
- keyboard の tooltip は locale の上書き（0005）の対象から外れる。
