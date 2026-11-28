# ADR 0033: 再生中の loop 再生キーは `jumpPlayback(0)` で先頭から鳴り直す

- 状態: 採用
- 決定者: user（キーで loop 再生を実行すること・連打しても音が重ならないこと・U1 を選んだこと・UAPMD の仕様を書き換えずアプリの要求はアプリ側で受け持つこと・キーと JS の対応を `keybinds.toml` に書き、薄い API を JS で組み立てること）。Surge XT の最初の音をアプリ側で直さないことは AI の提案
- 関連: [0032](0032-keybinds-in-repo-file.md)（`keybinds.toml`）

## 決定

- `keybinds.toml` の `[scripts]` にキーと `include` の path を書き、起動時の `init.js` が `repl.keys.config()` の `[scripts]` を `repl.keys.bind(key, () => include(path))` で結ぶ。Rust は押されたキーの JS を呼ぶだけ。
- `play-loop.js` は、止まっていれば `startPlayback`、再生中なら `uapmd.sequencer.jumpPlayback(0)` で先頭へ戻す（UX「U1」）。jump の flush で instrument の音と release は止まり、effect plugin の delay・reverb の tail は残る。
- UAPMD の jump・stop の flush（全 channel の CC120・CC123 と、鳴っている音の Note Off）は書き換えない。
- Surge XT が鳴り直しの最初の音を消す挙動は、アプリ側で直さない。

## 理由

- user が想定していた UX は、連打のたびに曲の先頭が鳴り直し、release・reverb tail も乗ったまま続く（live 演奏が続いているような）挙動だった。UAPMD の jump は instrument の鳴っている音を止める仕様で、それを保ったまま instrument の release を残す筋の良い API が無い。user はこの仕様を受け入れ、ETC を優先して U1 を選んだ。flush は instrument への MIDI なので effect plugin の tail は止まらず、user はそれが残る方がよいと判断した。
- 再生中の `startPlayback` は、鳴っている音に Note Off を送らずに先頭へ戻るので、連打すると同じ音の Note On が重なり、peak が上がり続ける（0.2 秒おき6回で 0.24 → 0.76）。jump は flush を伴うので重ならない。
- user の判断: UAPMD への補修は小さく薄くし、利用アプリの要求は利用アプリ側（Rust/JS）が責務として受け持つ。
- Surge XT は CC120 を受けると内部の 32 samples の block ごとに音量を下げ、8 block 後に全 voice を解放するので、CC120 から 256 samples 以内に始まった音も消える。jump の直後は曲の先頭の音がこの範囲に入る。Six Sines（CLAP）は同じ block の CC120 の後ろの Note On を鳴らすので、UAPMD は event の順序を守っており、Surge XT 側の挙動である。

## 却下した案

- **instrument の release を残したまま鳴り直す（UX「U3」）。** 今の UAPMD に筋の良い API が無い。
- **loop の終端を伸縮して鳴り直す。** user が却下。UAPMD の loop 処理の変更に弱い。
- **flush から CC120 を外す（または plugin ごとに間引く）UAPMD 補修。** user が却下。UAPMD の仕様をアプリの要求で書き換えることになる。
- **JS に file・toml を読む API を作る。** Rust が起動時に読んだ中身を `config()` で返せば足りる。

## 結果

- 連打しても同じ音の Note On は重ならない。鳴り直すたびに instrument の release は切れるが、effect plugin の tail（Surge XT Effects の delay・Dragonfly Hall Reverb で確認）は残り、先頭の音に重なる。
- Surge XT では、鳴り直しの最初の音（曲の先頭から 256 samples 以内に始まる音）が鳴らない。
- `[scripts]` は `keybinds.toml` の末尾に置く（toml では表の見出しより後ろのトップレベルの key がその表の中身になる）。
- JS から `repl.keys.bind` で後からキーを結べる。help の一覧に出るのは `keybinds.toml` に書いたキーだけ。
