# ADR 0029: native bridge は build.rs が build し、`scripts/build.py` は配布用の埋め込みだけを持つ

- 状態: 採用
- 決定者: user（repo の中で動かす物は `cargo build --release` だけで全部 build できるようにすること・cargo で書けない extension DLL の埋め込みは配布用の script に残すこと）。build.rs で CMake を直接呼ぶこと・configure を省く条件・監視する path は AI の設計
- 関連: [0003](0003-build-boundary.md)（repo の中の exe は disk から読む）、[0007](0007-gui-extensions-as-dll.md)（埋め込みの手順）

## 決定

- `build.rs` が `cmake` で `build/out/repl_bridge.dll` を build する。C/C++ の runtime は `CARGO_CFG_TARGET_FEATURE` の `crt-static` から決める（有れば /MT）。
- 明示的な configure は、`build/CMakeCache.txt` が無いときか、そこに記録された runtime が違うときだけ行う。それ以外は `cmake --build` だけを呼ぶ。
- build.rs を再実行させる path は `native/`・`patches/uapmd/`・`../uapmd` の `source/`・`external/`・`CMakeLists.txt`。
- `scripts/build.py` は、workspace を build して extension DLL を `build/out/extensions/` へ写し、もう一度 build して exe に埋め込む処理だけを持つ。

## 理由

- user の要件: Rust の project は `cargo build --release` だけで build できるのが単純で、開発体験が良い。native の依存を build.rs から build するのは `*-sys` crate と同じ Rust の慣習。
- 埋め込みは同じ workspace の cdylib に依存するので、stable の cargo では書けない（artifact dependencies は nightly 限定）。build.rs から cargo を呼ぶと build ロックで止まる。repo の中の exe は埋め込みを使わないので（0003）、埋め込みは配布物を作るときだけで済む。
- 明示的な configure は、補修の生成物を書き直して bridge を再 build するので、毎回約2分かかる。補修の入力は `CMAKE_CONFIGURE_DEPENDS` に入っているので、`cmake --build` が自分で configure し直す。何も変わっていないときの `cmake --build` は約3秒で終わる。
- `build/out/repl_bridge.dll` は監視しない。build.rs が自分で書くので、bridge を build し直すたびに次の build で build.rs と exe の compile がもう一度走る（約20秒）。exe の作り直しは `include_bytes!` の依存として rustc が追う。

## 却下した案

- **`cmake` crate を使う。** install 先と build dir を `OUT_DIR` に置く前提なので、既存の `build/` と、そこを指す補修の手順・回帰テストから離れる。
- **`cargo xtask` に移す。** 埋め込みを cargo の command に見せるだけで、2回 build する仕組みは変わらない。一方で、package が1つ増える。
- **埋め込みをやめて exe と DLL を zip で配る。** exe 1つで動く配布を失う（0007）。

## 結果

- `.rs` だけを変えたときは build.rs が走らず、CMake の時間は増えない。
- 監視していない path（`build/` の削除など）で DLL が無くなったときは、`native/` の file を touch すると build.rs が走る。
- CMake の出力は、build が失敗したとき、または `cargo build -vv` のときだけ見える。
