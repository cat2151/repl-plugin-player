# ADR 0030: audio device は入力を開かず、再生専用で開く

- 状態: 採用
- 決定者: user（AI が出した3案から (a) を承認。将来 audio 入力を使えなくなることを引き受けた）。補修の形（`device-buffer-size` の出力に重ねる別の補修）は AI の設計
- 関連: `patches/uapmd/playback-only-device/README.md`（原因・適用条件・撤去条件）

## 決定

- UAPMD の `MiniAudioIODevice::onOpen` は、入力 device が指定されていれば duplex で開く。これを補修 `playback-only-device` で常に playback だけで開くようにする。

## 理由

- user 報告の症状: 起動後の `play("cde")` で、毎回最初の c が途中で切れる。
- duplex だと miniaudio は入力を 1 period（1024 frames）読んでから出力を書くので、出力の書き込みが入力の到着（20ms か 30ms ごと）に縛られる。play のたびに device を起動し直すので、起動直後は再生 buffer の貯金が少ない。起動から約 140ms で、Windows が 10ms ごとに読む 480 frames に 32 frames 足りなくなり、その分の無音が差し込まれていた。
- repl-plugin-player には audio 入力を読む機能が無い（`src`・`uapmd-host`・`snippets`・`tests`・`patches` で 0 件）。入力を閉じても、今は何も失わない。

## 却下した案

- **device の起動直後に、再生 buffer へ無音を先に詰める。** miniaudio の duplex loop の中を書き換える補修になり、(a) より広い。
- **play のたびに device を止めない。** `tutorial.js` の `stop()` と AppModel の shutdown の意味が変わり、(a) より広い。

## 結果

- 回帰テスト: `python scripts/first-note-dropout-test.py`（loopback 録音を使う。一括の `scripts/e2e.py` には入れていない）。
- マイクなどの入力を effect に通す使い方を始めると、入力は無音になる。そのときは、この補修を外すか、入力 device を明示したときだけ duplex で開く形に変える。
- `include` は `device-buffer-size` の次の行に置く。順序が違うと configure が止まる。
