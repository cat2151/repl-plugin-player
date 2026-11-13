# ADR 0006: 画面全体を `egui_tiles` の tile にし、フローティングは immediate viewport の OS window にする

- 状態: 採用
- 決定者: user（REAPER の ReaImGui と同じく拡張や既存の欄を本体にドッキングすること・フローティングは本体の外の OS window にすること・最も単純なやり方として `egui_tiles` を使うこと・対象を画面全体にし editor も1つの tile にすること・`[panes]` の幅の下限を廃止して黙って無視すること・本体の外の OS window を dock window にして keyboard などの欄をドッキングし、中の配置を tile の drag で変えられるようにすること）。`egui_tiles` の版の固定・欄の切り方・最初の配置・immediate viewport・タブの menu と button での行き来・`repl.ui` の窓の出る場所と閉じ方・配置を保存しないこと・dock window の id と題名の決め方・dock window の button が窓の欄を全部本体へ戻すこと・dock window のタブに menu を出さないことは AI の提案
- 関連: [0003](0003-build-boundary.md)（E2E の cell は `.js` に置く）

## 決定

- 本体の画面は `egui_tiles`（`=0.18.0`）の tree 1つにする。欄は editor・main（見出し・結果・timeline など）・keyboard・scope・log・reference と、`repl.ui` の窓1枚ずつ。どの欄もタブを持ち、drag で分割・移動できる。
- 欄は本体の tree の中か、本体の外の OS window（dock window）の tree の中のどちらかにいる。dock window も `egui_tiles` の tree を1つずつ持ち、中の欄は drag で分割・移動できる。OS window は `Context::show_viewport_immediate` で出す。本体のタブの右 click の menu で欄1つの dock window へ出し、dock window の中の button でその窓の欄を全部、本体の active なタブの列へ戻す。dock window のタブには menu を出さない。
- dock window の viewport の id は、窓ができたときの最初の欄から決める。同じ欄を再び出すと、前の窓の場所に出る。題名は中の欄の題名を ` / ` でつないだ物。
- `repl.ui` の窓は、初めて出るときは OS window。どの窓があるかは registry（`crate::script_ui`）が決め、tree と OS window はそれに毎 frame 合わせる。タブの × と OS window の × は registry から消す。既存の欄は閉じられず、OS window の × は本体へ戻す。
- `config.toml` の `[panes]`（`left_min_percent`・`right_min_percent`）は読まない。残っていても黙って無視する。
- 配置の保存は [0008](0008-layout-in-config-or-auto-file.md) に置き換えた。

## 理由

- user の要件: 拡張も既存の欄も、本体にドッキングでき、外にも出せる部品にしたい。将来は editor も差し替えの対象になりうるので、editor だけを特別な panel に残さない。
- deferred viewport は closure に `Send + 'static` が要るが、欄の描画は `Editor` が持つ `Control`（JS の評価）を借りる。immediate viewport なら同じ frame の中で `&mut Editor` を貸したまま描ける。
- `egui_tiles` にはタブを drag して本体の外へ出す機能が無い。menu と button の2操作なら `egui_tiles` に手を入れずに行き来できる。
- `repl.ui` の窓の有無を registry 1か所に持たせると、同じ id の再 `show` は場所を変えず、`repl.ui.close` はタブでも OS window でも次の frame で消える。閉じる経路が registry の `close` 1本になる。
- `[panes]` は左右2列の固定の分割で幅の下限を守る設定で、tile の境界を user が drag で動かせる画面では守る対象が無い。利用者は作者だけなので、移行や notice は作らない。

## 却下した案

- **`egui_dock`。** 当時の最新（0.21.1）が egui `^0.36` で、この repo の egui 0.37 では使えない。
- **`../cat-smf-editor` 式の自前の配置（`layout.toml` に書いた配置で pane・タブ・別の OS window を出す）。** 本体だけで約2600行（`src/view/dock/` 約1400行、`src/store/layout*` 約1200行）で、egui も 0.31 と版が違い、そのまま持ち込めない。user の「最も単純な物」に合わない。
- **フローティングを本体の窓の中に重なる `egui::Window` にする。** user の要件（本体の外の OS window）に合わない。
- **deferred viewport。** 上の `Send + 'static` の制約で、欄の描画から `Control` を借りられない。

## 結果

- headless の `Context`（`embed_viewports` が既定で `true`）では、immediate viewport は callback 無しで本体の中の埋め込みの窓になる。headless の回帰はこの埋め込みの窓を「OS window の場所」として扱い、OS window の × は eframe の実画面でしか確かめられない。
- 欄の境界を drag で動かせる代わりに、欄の下限の高さは守られない。
- tree どうしは独立なので、本体と dock window の間・dock window どうしの間では欄を drag で移せない。移すには、いったん本体へ戻す。
- snippet 一覧・設定・help・起動中の overlay は tile にせず、今までどおり浮動の窓のまま。
