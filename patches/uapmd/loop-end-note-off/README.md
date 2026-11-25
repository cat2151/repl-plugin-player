# loop終端のNote Off

## 原因と適用条件

検証した上流revision: `9f708d3300b4a6f8109eee0fadccf286db8b7a45`。
上流はloop終端で描画を分割するとき、鳴っている音のNote Offを送る
（`processAudioForRenderSegment` の `sendNoteOffsAtEnd`）。ただし
`TimelineFacadeAudio.cpp` の判定が `>` なので、音声ブロックの末尾がloop終端に
ちょうど一致すると分割とみなされない。次のブロックの先頭へのseekが
鳴っている音の記録を消すので、Note Offはどの周回でも送られず、音が残る。

判定を `>=` にして、一致する場合も上流のNote Off送出を通す。
Note Offが終端より後にある音も、上流の処理によって終端で止まる。
音符・SMF・保存形式・loop周期は変えない。

## 再適用と撤去

依存checkoutを変更しない。`native/CMakeLists.txt` のincludeが正本。
`generate.py` が `TimelineFacadeAudio.cpp` のコピーを `target/shim/loop-end-note-off/` に
生成し、`uapmd-engine` のビルド対象を差し替える。このコピーは
`frozen-loop-playback` の入力になるので、includeはそれより前に置く。
通常の `python scripts/build.py` で既存キャッシュ・再cloneの双方に適用される。

適用失敗時は補修名、対象ファイル、anchorの不一致条件、上流revision、
この説明書の場所を表示して停止する。上流のloop分割の条件を調べ、
同等の修正が入ったかを先に確認する。検査を外してビルドしない。
上流の素の `TimelineFacadeAudio.cpp` で下記の回帰テストが通ったら、includeと
このdirectoryを撤去し、`frozen-loop-playback` の入力を上流のファイルに向ける。

## 検証

`regression.cpp` は実engineでloop終端より長いclipを再生し、trackの入力に届く
Note On/Offを数える。終端がブロック境界に一致する場合（開始0と非0）と、
ブロックの途中にある場合を調べる。素の上流では前の2つがNote Off 0回で失敗する。

```
cmake --build build --config Release --target loop_end_note_off_regression
build/out/loop_end_note_off_regression.exe
python scripts/e2e.py loop loop-update loop-replaces-clips
```

`tests/e2e/loop.js` は実プラグイン入力で複数周の末尾Note Offと、
既存のNote On列・周期・音声出力・clip不変を検査する。

## 作者への報告

未報告。`upstream-report.md` は送付用の下書きであり、外部送信はしていない。
