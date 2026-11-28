# ADR 0032: 操作のキーは repo の `keybinds.toml` に書き、修飾キーは完全一致で判定する

- 状態: 採用
- 決定者: user（keybind を抽象化して設定で変えられるようにすること・段階的に進めること・設定ファイルを repo で管理すること・開発体験をよくする方向にすること）。置き場所を `locales/` と同じ扱いにすること・修飾キーを完全一致にすること・起動時に1回だけ読むこと・読めない項目は埋め込みのキーに戻して赤字で出すこと・help と案内文のキー表記をこのファイルから取ること・重複を効く範囲ごとに数えること・Shift の例外は AI の提案
- 関連: [0003](0003-build-boundary.md)（repo の中の exe は `.toml` を disk から読む）、[0005](0005-locale-overlay.md)（repo の外の locale の重ね合わせ）

## 決定

- 操作ごとのキーは repo 直下の `keybinds.toml` に `操作名 = "Ctrl+Shift+P"` の形で書く。repo の中の exe は disk から、外の exe は埋め込みから読む（`resources::Source`）。repo の外で user が上書きする仕組みは持たない。
- 起動時に1回だけ読む。読めない項目・無い項目は埋め込みのキーを使い、未知の操作名・同じキーの重複とあわせて main の欄に赤字で出す。
- 重複は、同時に効きうる操作の間でだけ数える（操作ごとに効く範囲を持つ: `src/keybinds.rs` の `Scope`）。editor の PageUp と Snippets 窓の PageUp のように、同時に効かない操作は同じキーを持てる。
- 修飾キー（Ctrl・Shift・Alt）は書いた物と押した物が完全に一致したときだけ効く（egui の `matches_exact`）。例外として、editor へ戻るキーと editor の page のキーは、書いたキーに Shift を足しても効く。
- キーは配列で決まる文字で比べる（Ctrl+Alt で winit が US 位置の名前を返しても、JIS の `[` `]` は `[` `]` として効く）。
- 効いたキーが打つはずの文字は editor に入れない。
- help の一覧と editor の案内文は、このファイルに書いた文字列をそのまま出す。

## 理由

- user の要件: 設定は repo で管理し、user が作者1人なので開発体験を優先する。`locales/` と同じ扱いにすれば、キーを変えても build が要らず再起動だけで済み、置き場所の判定も `source_root` の1本のまま。
- egui 標準の `consume_key`（`matches_logically`）は余分な Shift・Alt を無視するので、`Shift+Enter` と `Ctrl+Shift+Enter` を別の操作に割り当てると、調べる順番で結果が変わる。完全一致なら設定の行の順に意味が無くなる。
- Shift の例外は、既定のキーの感触を変えないため（user の判断: 設定化と割り当て変更を混ぜない）。page は Shift で選択を伸ばし、Shift+Tab でも editor へ戻っていた。
- 重複を範囲なしで数えると、同時に効かない操作どうしが既定のキーのまま赤字になる。
- 読めない項目でアプリを止めると、release の exe は console を持たず（`windows_subsystem`）理由が見えない。項目ごとに既定へ戻せば、残りの操作はそのまま使える。
- help と案内文を同じファイルから取らないと、キーを変えた瞬間に画面の説明が嘘になる。

## 却下した案

- **egui の `matches_logically` のまま。** 割り当てを変えると、どの操作が先に調べられるかで効くキーが変わる。
- **変更をその場で読み直す（ホットリロード）。** 再起動で足り、監視の仕組みを足すほどの差が無い。
- **`config.toml`（`%LOCALAPPDATA%`）に置く。** user の要件（repo で管理）に合わない。
- **読めないファイルで起動を止める（`locales/` の厳密読み）。** release の exe では理由が見えない。

## 結果

- `Ctrl+Shift+Enter` のような「書いたキー＋余分な修飾キー」は、以前は効いたが今は効かない。
- repo の外の exe ではキーを変えられない。
- Snippets 窓の中の ↑↓・Enter なども完全一致になり、Shift+↓ などでは動かない。
- editor の外の Tab・Shift+Tab は、editor へ戻るキーを別にしても飲み込む（egui の Tab による widget 巡回を起こさない）。
- egui の TextEdit の中のキー（カーソル移動・Ctrl+Z・Tab の字下げ）と modal の Escape は変えられない。
