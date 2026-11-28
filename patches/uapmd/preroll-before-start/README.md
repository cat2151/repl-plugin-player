# 曲頭より前のprerollを描かない

## 原因と適用条件

検証した上流revision: `277aa85468795267d6cf64953af587ea18c7f653`。
latencyのあるtrackが1つでもあると、再生開始時にengineは描画位置を
可聴位置よりpreroll分（`alignToQuantum(maxRenderLeadInSamples())`）だけ前、
つまり負の位置から始める。`TimelineFacadeAudio.cpp` の `processTracksAudio` は
trackの描画開始位置が負のとき0に丸めるので、prerollの間のブロックはどれも
曲頭 `[0, ブロック長)` を描き直す。そのため曲頭のNote Onが、prerollの
ブロック数だけ余分にpluginへ届く。prerollの出力はengineが無音にするが、
pluginは同じ音を何度も受けて鳴り始めているので、最初の音だけ音色が変わる。

丸めをやめ、負の描画開始位置をそのまま `processAudioForRenderSegment` に渡す。
clipの描画窓（`computeClipRenderWindow`）は負の区間と重ならないので、
ブロック全体が負なら何も描かれず、途中で0をまたぐブロックは0の位置から
1回だけ描かれる。trackの入力バッファはブロック全体が0で埋められてから書かれるので、
前のブロックの音声は残らない。prerollの長さ・音符・SMF・保存形式は変えない。

負の位置からの描画・0への丸め・ブロックごとの描画は、どれもuapmdのengineの中にある
（`LatencyCompensationManager.cpp:491-496`、`TimelineFacadeAudio.cpp:132-133`）。
このアプリは `startPlayback()` を1回呼ぶだけなので、直すべき場所は上流で、
この補修は上流で直るまでのつなぎである。丸めは、描画位置が負になり得なかった頃の
何もしない防御の行で、prerollが入った後に負の位置を丸めるようになった。

prerollの切り上げ単位は、`SequencerEngine.hpp:26` の `kAudioBlockCapacityInFrames = 8192` を
`RealtimeSequencer.cpp:25` が `std::max(アプリのbuffer size, 8192)` でengineへ渡した値である。
そのためlatencyが32 samplesでもprerollは8192（約170 ms）になり、play開始が約170 ms遅れる。
このアプリのAPIからは変えられない。この補修では直さない。
[未検証] prerollを縮めると、Surge XTなどで最初の音が消えるかもしれない
（cat-plugin-playerのADR 0008・0018は、発音前に空のイベントで100 ms処理させて
最初のnote-onの無音を防いでいる。今は8192のprerollが同じ下準備の働きをしている）。
[未検証] 負の描画位置で、pluginへ渡るtransportの位置も負になるか。

## 再適用と撤去

依存checkoutを変更しない。`uapmd-host/CMakeLists.txt` のincludeが正本。
`generate.py` が `target/shim/frozen-loop-playback/TimelineFacadeAudio.cpp` から
`target/shim/preroll-before-start/TimelineFacadeAudio.cpp` を生成し、
`uapmd-engine` のビルド対象を差し替える。入力が `frozen-loop-playback` の生成物なので、
includeはそれより後に置く。通常の `python scripts/build.py` で既存キャッシュ・
再cloneの双方に適用される。

適用失敗時は補修名、対象ファイル、anchorの不一致条件、上流revision、
この説明書の場所を表示して停止する。上流で丸めの周辺が変わっていないか、
同等の修正が入ったかを先に確認する。検査を外してビルドしない。
上流の素の `TimelineFacadeAudio.cpp` で下記の回帰テストが通ったら、includeと
このdirectoryを撤去する。

## 検証

`regression.cpp` は実engineに、latency 32のpluginを持つtrackとlatency 0のtrackを置き、
両方に曲頭のNote Onを持つclipを置いて再生する。各trackの入力に届くNote Onを数え、
1回なら通る。engineのbuffer容量1024、ブロック256なのでprerollは4ブロックで、
補修なしではlatency 0のtrackが5回、latency 32のtrackが4回で失敗する。

```
cmake --build build --config Release --target preroll_before_start_regression
build/out/preroll_before_start_regression.exe
```

## 作者への報告

未報告。`upstream-report.md` は送付用の下書きであり、外部送信はしていない。
