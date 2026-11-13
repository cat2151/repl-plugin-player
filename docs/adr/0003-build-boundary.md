# ADR 0003: build が要るのは `.rs` と `native/` だけにし、JS は `.js` ファイルにだけ書く

- 状態: 採用
- 決定者: user（JS だけを調べたり直したりするときに release build を回さないこと・build が要るのは `.rs` と `native/` だけにすること・JS は `.js` ファイルにだけ書くこと・規則の明文化と E2E のシナリオの JS 化と埋め込み JS の disk 読みを全部入れること）。配布用の exe は埋め込みのままにすること・disk から読むかの判定を1本にすること・E2E の runner の形・観測 API の形は AI の提案
- 関連: [0002](0002-cells-address-instruments-by-track.md)（init script と lib の JS）

## 決定

- repo の中で動く exe は、`.js`（snippet・`src/*.js`・E2E の case）と `.toml`（locale）を常に disk から読む。repo の外の exe は、今までどおり build 時に埋め込んだ物を使う。
- disk から読むかの判定は `src/resources.rs` の `source_root`（exe の祖先に package 名 `repl-plugin-player` の `Cargo.toml` がある dir）1本にする。repo の中の exe でファイルが無ければ、埋め込みへ戻らずエラーにする。
- E2E のシナリオは `tests/e2e/<case>.js`（uapmd の補修の回帰は `patches/uapmd/<補修名>/<case>.js`）に `// %% <title>` の cell で書き、`--e2e <case>` が disk から読んで cell を1つずつ流す。title の語 `init`・`stop`・`error` で runner の扱いを変える。補助関数は `tests/e2e/lib.js` に置く。
- Rust に残すのは、起動・終了の case と、検査対象が Rust のコード（projection・scope 表示・範囲の切り出し・editor・egui・代役の patch browser）の case だけ。その case でも JS は case の `.js` の cell に置き、Rust は title で引いて流す。値を渡す cell は関数式にして、Rust は JSON を引数に呼ぶ。
- JS から読めない native の観測は、既存の bridge command と同じ関数を呼ぶだけの `repl.test.*`（`observe`・`notes`・`loopTrace`・`loopChecks`・`pluginState`）で出す。

## 理由

- user の要件: JS の変更を確かめるたびに release build を回すのが一番の無駄で、JS と Rust の境界を早いうちから単純なまま保ちたい。
- 配布用の exe は必ず build してから置くので、埋め込みと disk の食い違いが起きない。
- 判定を `project_root` 系にそろえたのは、`cargo test`（`target/debug/deps` から動く）が snippet を disk から読めることを保つため。`target/release` に限る判定にそろえると、テストが snippet を読めなくなる。
- source tree があるのにファイルが無いときに埋め込みへ戻ると、直した `.js` が効かない理由が見えなくなる。
- runner を `--e2e` の側に置くと、`scripts/e2e.py` の今の検査（`e2e_pass`・`cleanup`・NoteOn の数）と、起動・終了の case と同じ入口・同じ log を保てる。
- Rust の値を文字列で JS に埋め込まず関数式に JSON を渡すと、cell の中身が `.js` のまま読め、Rust 側に JS の断片が残らない。

## 却下した案

- **新しい E2E だけを JS にする。** 古い物と新しい物で境界が2本になり、どちらを直すと build が要るかを毎回考えることになる。
- **`scripts/e2e.py` が `--automation` に `.js` を流す。** `--automation` にも `api`・`cleanup` の log は出るが、合否と cell の分割・Stop を Python 側に持つことになり、runner が Rust と Python の2か所になる。
- **disk から読むのを `target/release` の exe に限る。** `cargo test` が snippet を disk から読めなくなる。

## 結果

- `.js`・`.toml` だけを変えたときは build しない（`AGENTS.md` の「build の境界」）。runner 自体や、Rust が title で引く cell の title を変えたときは build が要る。
- repo の中の `target/debug` や repo 直下に置いた exe も、locale を LocalAppData でなく source tree から読む。repo の外の exe の locale は、埋め込みに LocalAppData の user のキーを重ねる（[0005](0005-locale-overlay.md)）。
- `src/*.js` の変更は次の起動から効く（`resources::Source` はプロセス内で最初に使ったときに1回だけ読む）。
- `uapmd.instance(i).saveState(file)` は上流のままでは返らないので、plugin の state をファイルへ書いて読み戻す state-load の case は Rust に残る。
