# ADR 0033: 再生中の loop 再生キーは stop → 1 block 待つ → start で先頭から鳴り直す

- 状態: 採用
- 決定者: user（キーで loop 再生を実行すること・連打しても音が重ならないこと・U1 を選んだこと・UAPMD の仕様を書き換えずアプリの要求はアプリ側で受け持つこと・キーと JS の対応を `keybinds.toml` に書き、薄い API を JS で組み立てること・再生中は stop → 待つ → start にし、待つのを壁時計でなく audio block が進んだことにすること・engine を止めないこと）。待つ block 数を 1 にすること・1 秒進まなければ error にすること・既存の `__remidy_audio_worker_diagnostics()` で block 数を読むことは AI の提案
- 関連: [0032](0032-keybinds-in-repo-file.md)（`keybinds.toml`）

## 決定

- `keybinds.toml` の `[scripts]` にキーと `include` の path を書き、起動時の `init.js` が `repl.keys.config()` の `[scripts]` を `repl.keys.bind(key, () => include(path))` で結ぶ。Rust は押されたキーの JS を呼ぶだけ。
- `play-loop.js` は、止まっていれば `startPlayback`。再生中なら `stopPlayback` を呼び、`__remidy_audio_worker_diagnostics().timingCounters.realtimeBlocks` が 1 つ進むまで `repl.processEvents()` を回してから `startPlayback` を呼ぶ（UX「U1」）。block が 1 秒進まなければ error を投げる。
- engine（`setEngineEnabled`）は止めない。stop の flush で instrument の音と release は止まり、effect plugin の delay・reverb の tail は残る。
- UAPMD の jump・stop の flush（全 channel の CC120・CC123 と、鳴っている音の Note Off）は書き換えない。

## 理由

- user が想定していた UX は、連打のたびに曲の先頭が鳴り直し、release・reverb tail も乗ったまま続く（live 演奏が続いているような）挙動だった。UAPMD の stop・jump は instrument の鳴っている音を止める仕様で、それを保ったまま instrument の release を残す筋の良い API が無い。user はこの仕様を受け入れ、ETC を優先して U1 を選んだ。flush は instrument への MIDI なので effect plugin の tail は止まらず、user はそれが残る方がよいと判断した。
- 再生中の `startPlayback` だけでは、鳴っている音に Note Off を送らずに先頭へ戻るので、連打すると同じ音の Note On が重なり、peak が上がり続ける（0.2 秒おき6回で 0.24 → 0.76）。stop は flush を伴うので重ならない。
- Surge XT は CC120 を受けると内部の 32 samples の block ごとに音量を下げ、8 block 後に全 voice を解放するので、CC120 から 256 samples 以内に始まった音も消える。`jumpPlayback(0)` は flush と曲の先頭の音が同じ範囲に入り、先頭の音が鳴らない。Six Sines（CLAP）は同じ block の CC120 の後ろの Note On を鳴らすので、UAPMD は event の順序を守っており、Surge XT 側の挙動である。
- flush と先頭の音の間を host の audio block 1 つ分空ければ、Surge XT の 256 samples を越える。壁時計で待つと audio thread の進み方に左右され、20ms 待ちでも 6 回中 1 回は鳴らなかった。block が 1 つ進むまで待つと 6 回中 6 回鳴った。
- user の判断: UAPMD への補修は小さく薄くし、利用アプリの要求は利用アプリ側（Rust/JS）が責務として受け持つ。block 数は UAPMD の既存の JS 関数が返すので、新しい API は足さない。

## 却下した案

- **再生中も `jumpPlayback(0)` で先頭へ戻す。** Surge XT で先頭の音が鳴らない。
- **stop の後、壁時計で待ってから start する。** 待ち時間を決められない（20ms でも外れた）。
- **engine を止めてから start する。** effect plugin の tail が切れる。
- **`repl.audio.blocks()` を新設する。** `__remidy_audio_worker_diagnostics()` が同じ counter を返すので、wrap が要らない。
- **instrument の release を残したまま鳴り直す（UX「U3」）。** 今の UAPMD に筋の良い API が無い。
- **loop の終端を伸縮して鳴り直す。** user が却下。UAPMD の loop 処理の変更に弱い。
- **flush から CC120 を外す（または plugin ごとに間引く）UAPMD 補修。** user が却下。UAPMD の仕様をアプリの要求で書き換えることになる。
- **JS に file・toml を読む API を作る。** Rust が起動時に読んだ中身を `config()` で返せば足りる。

## 結果

- 連打しても同じ音の Note On は重ならない。鳴り直すたびに instrument の release は切れるが、effect plugin の tail（Surge XT Effects の delay・Dragonfly Hall Reverb で確認）は残り、先頭の音に重なる。
- Surge XT でも、loop 再生キーで鳴り直した先頭の音が鳴る。押してから鳴り直すまで block 1 つ分（48kHz・1024 frames で約 21ms）遅れる。
- `play-loop.js` 以外で再生中に `jumpPlayback` する経路では、Surge XT の先頭の音は今も鳴らない。
- 再生中に押して audio block が 1 秒進まないとき（audio device が処理していないなど）は、loop 再生キーが error を出し、再生は止まったままになる。
- `[scripts]` は `keybinds.toml` の末尾に置く（toml では表の見出しより後ろのトップレベルの key がその表の中身になる）。
- JS から `repl.keys.bind` で後からキーを結べる。help の一覧に出るのは `keybinds.toml` に書いたキーだけ。
