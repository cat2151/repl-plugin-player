# ADR 0007: GUI の拡張は QuickJS と DLL の2本にし、DLL は同じ workspace で build して `egui::Ui` を直接渡す

- 状態: 一部置き換え済み（[0011](0011-extensions-cloned-into-workspace.md)）
- 決定者: user（GUI の拡張の方式を即時性の QuickJS と速さの DLL の2本にすること・DLL の土台を作ること・wasm をやめること・既存の native の Rust 機能のうち小さい物から DLL にし最初は keyboard 表示にすること・増やす package は DLL の1つだけで test を持たせないこと（共通の crate を作らないことは [0011](0011-extensions-cloned-into-workspace.md) で置き換え）・本体と DLL の egui をそろえるため root を cargo workspace にすること）。package の場所と名前・`default-members`・DLL の口の形・読込の場所と unload しないこと・読めないときの赤字・exe への埋め込みと `scripts/build.py` の手順・状態の確認用の口は AI の提案
- 関連: [0003](0003-build-boundary.md)（repo の中か外かの判定 `source_root`、埋め込みへ黙って戻らないこと）、[0006](0006-dock-with-egui-tiles.md)（keyboard の欄）

## 前提

- user の方向: UAPMD などの core 以外のほぼ大部分を、差し替えられる部品にする。repl window（script editor）そのものも対象になりうる。部品はすべて本体にドッキングされるのが理想。
- user の線引き: core = uapmd + core の操作の一覧。JS はその入口の1つで、速さが要る所には native の入口も持つ。部品はすべて同じプロセスに置く。

## 決定

- GUI の拡張は、即時性の QuickJS（`repl.ui` などの JS）と、速さの DLL の2本にする。JS で遅くなった物を DLL にする。
- DLL は root の cargo workspace の member（`extensions/<名前>/`、`crate-type = ["cdylib"]`、`[lib]` に `test = false`・`doctest = false`）にし、`default-members` に入れて `cargo build --release` で本体と一緒に build する。egui は `[workspace.dependencies]` の `eframe` を本体と DLL の両方が引く。
- DLL の口は Rust の呼び出し規約の `#[no_mangle] pub fn`（C ABI にしない）で、`&mut egui::Ui` を直接受け取る。文言など本体にしか無い物は引数で渡す。
- 本体は最初に描く frame で `libloading` で1回読み、最後まで unload しない。repo の中の exe は自分の profile の dir（`target/release` など。test の実行ファイルなら `deps` の親）の DLL を読み、repo の外の exe は exe に埋め込んだ bytes を `%LOCALAPPDATA%` の下へ書き出して読む。
- DLL が読めないときは、その欄に path と理由を赤字で出し、本体はそのまま動く。他の場所や埋め込みへ黙って戻らない。
- 配布の zip は exe 1つのまま。`scripts/build.py` が workspace を1回 build して DLL を `build/out/` へ写し、もう1回 build して exe に埋め込む。
- 最初の DLL は keyboard 表示（今は `extensions/builtin` の `builtin/keyboard`）。DLL の名前・口・on/off は [0011](0011-extensions-cloned-into-workspace.md)。

## 理由

- user の要件: 実験的な機能、特に GUI を含む物を、本体を小さいまま足して UX 検証や概念実証をしたい。ゆくゆくは別 repo で 3rd party が DAW の UI やワークフローを作る材料にする。即時性の端は QuickJS、速さの端は DLL で埋まる。
- 同じプロセスに読む DLL には実績がある（REAPER の SWS、uapmd-app の addin。addin は同じ compiler 前提の C++ から共有の ImGui を直接呼ぶ、この決定と同じ型）。
- Rust の DLL に安定した ABI は無く、egui は widget の状態を `TypeId` で引いて `Context` に保存する。同じ workspace・同じ `Cargo.lock`・同じ feature の解決で build すれば両側の型がそろい、DLL が保存した `ScrollArea` の状態を本体が読め、次の frame でも保たれることを `cargo test`（`src/extension/tests.rs`）で確かめた。
- DLL の package に test を持たせないので、`cargo test` は DLL を compile せず、test の build 時間はほぼ変わらない（`src/main.rs` を触った後の `cargo test --release --locked --no-run` が前 12.3 秒、後 12.6〜13.8 秒）。兄弟 repo の clap-mml 系で crate を増やして test の build が長くなったことへの user の対処。

## 却下した案

- **wasm。** 即時性と速さの両端が QuickJS と DLL で埋まり、wasm にしかない利点（本体が落ちない・安全な差し替え・配布と sandbox）は今は要らず、この用途の実績も無い（user の判断）。この意見は将来変わる可能性がある。
- **別プロセス。** 部品を本体にドッキングできない。
- **機能ごとに crate を分けて静的 link する。** crate ごとに test の build が増える。
- **C ABI。** `egui::Ui` を渡せず、境界を一から設計することになる。3rd party に開くときに考える。

## wasm をやめて失う物

- DLL の差し替えにくさ（Windows の file lock、unload したときの危険）を埋める物が無い。DLL は差し替えず、最後まで unload しない。
- `egui::Ui` を直接渡す形は egui そのものが API なので、wasm に戻るなら境界を設計し直すことになる。

## 結果

- DLL と本体の egui がそろっていないと、テストが失敗するのでなく、DLL が egui の状態を引いた所で panic し、`Rust cannot catch foreign exceptions` で process ごと落ちる。
- `cargo build -p <DLL の package>` だけを走らせると、その回に選ばれた package だけで feature が解決され、本体と違う egui を持つ DLL が `target/release/` に上書きされる（workspace 全体で feature をそろえる cargo の設定は unstable）。DLL は `-p` を付けずに本体と同じ回で build する。`scripts/package-release.py` は、埋め込む `build/out/` の DLL が `target/release/` の物と同じ bytes であることを確かめる。
- `include_bytes!` で `build/out/` の DLL を直接埋め込むと、DLL は cargo 自身が作る物なので新しい clone の1回目の `cargo build` が通らない。`build.rs` が、DLL があれば埋め込み、無ければ空にし、repo の外の exe は空なら赤字を出す。
- keyboard の DLL は約 3.9 MB（CRT を静的に link した配布用で約 4.0 MB）。DLL が無いときは keyboard の欄に `<path>: LoadLibraryExW failed` が赤字で出て、cell の実行など他の部分は動く。
