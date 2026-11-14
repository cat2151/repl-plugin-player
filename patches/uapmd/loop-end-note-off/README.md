# loop終端のNote Off

## 原因と適用条件

UAPMD `4ec5c3b1c30130fdd3b7d4b375082273247e561f` で確認した。
`TimelineFacadeAudio.cpp` はloop終端で描画を分割して先頭に戻す。
`MidiClipSourceNode::processEvents` は終端を含まないため、終端ちょうどの
Note Offがどの周回でも送られず、末尾の音が残る。

loop終端まで届くMIDIクリップの処理中だけthread-local scopeを有効にし、
終端ちょうどのNote Offを最後のサンプルに収めて送る。
MIDI 1.0/2.0のNote Offとvelocity 0のNote Onだけを対象にする。
同時刻の通常Note On、program change、終端より後のイベントは含めない。
通常再生の終端除外とloop周期は変えず、音符・SMF・保存形式も変更しない。
Note Off自体がloop終端より後にある音の強制切断は、この補修の対象外。

## 再適用と撤去

依存checkoutを変更しない。`native/CMakeLists.txt` のincludeが正本。
`generate.py` が2つのcppのコピーを `target/shim/loop-end-note-off/` に生成し、
`uapmd-data` のビルド対象だけ差し替える。既存の補修とは独立している。
通常の `python scripts/build.py` で既存キャッシュ・再cloneの双方に適用される。
ソースと生成処理の変更時はCMakeが再構成する。

適用失敗時は補修名、対象ファイル、anchorの不一致条件、上流revision、
この説明書の場所を表示して停止する。上流差分とloop分割・MIDI送出を調べ、
同等の修正が上流に入ったかを先に確認する。検査を外してビルドしない。
上流が終端releaseの送出を保証し、下記テストが補修なしで通ったら、
includeとこのdirectoryを撤去する。

## 検証

`regression.cpp` は実際の生成済みMidiClipSourceNodeを使い、通常の終端除外、
同時刻にNote Onが先行する場合、program change・範囲外releaseの除外、
MIDI 1.0/2.0とvelocity 0、offset、重複送出とscope漏れを確認する。

```
cmake --build build --config Release --target loop_end_note_off_regression
build/out/loop_end_note_off_regression.exe
python scripts/e2e.py loop loop-update loop-replaces-clips
```

`tests/e2e/loop.js` は実プラグイン入力で複数周の末尾Note Offと、
既存のNote On列・周期・音声出力・clip不変を検査する。

## 作者への報告

未報告。`upstream-report.md` は送付用の下書きであり、外部送信はしていない。
