# プラグイン入力ノートと出力波形の観測

## 目的と適用範囲

右paneのミニ鍵盤で、全プラグインへ渡されたNote On/Offの和集合を表示する。
公開Timeline APIで読める保存イベントやスクリプトの呼び出し履歴だけでは、
エンジン停止中のキュー、Stop時のflush、実際の再生時点を判別できない。
確認したUAPMDにはプラグイン入力ノートの公開観測APIがないため、入力直前へ
読み取り専用の観測点を追加する。既存の発音処理の修正ではなく、表示用の拡張である。

右paneのオシロスコープは、同じ準備済みSurge XTの処理成功直後にも観測点を持つ。
実際の入力UMPと第1出力busのL/RをRustへ渡す薄いcallbackで、monoは両chへ複製する。
対象は音源単体の出力であり、後段effectや他trackを含むmaster mixではない。
Float32/64に対応し、音声threadでは確保・ロック・JS実行をしない。
callbackは音源作成前に登録し、エンジンの停止・破棄後までRust側の有界queueを保持する。

検証した上流revision: `a4a96938eb31ddcb70209d62caf0dcc6ea9d3abe`。
適用対象は `source/uapmd-graph/src/node-graph/AudioPluginNodeImpl.hpp` の
`trackInputEvents(process.eventIn())` 直後。linear/full DAG両方で共通の観測点を使う。
依存checkoutは書き換えず、CMake構成時にコピーを `target/shim/note-monitor` に生成する。
再clone後も通常の `python scripts/build.py` で生成する。既存ビルド領域ではCMakeの変更検知により
再構成し、上流ヘッダやgraphソース変更時もコピーを更新する。

## 表示とスレッド境界

Timelineの全ノートを含む音域の鍵盤を、Timelineのスクロール領域の上に固定する。Note Onは緑色で点灯し、
Note Offで消灯する。ノート番号は文字で表示しない。
鍵盤は表示専用。準備中と観測失敗は全消灯になる。

各ノードが固定長配列でgroup/channel/pitchを追跡し、構築時に確保した自分のslotへ
lock-free atomicで公開する。GUIはフレームごとに全slotの和集合を読み、ノート状態が変わった場合だけ
audio threadが公開値を更新する。slotは64個で、それを超えたノードは鍵盤に出ない。
準備済みSurge XTだけを対象にすると、他の音源を鳴らすsnippetで鍵盤が点かない。
audio threadにはロック・動的確保・JS実行を追加しない。
MIDI 1/2、velocity-zero Note On、CC120/123を扱う。複数チャネルの同音は合成し、
一方の解除で他方の点灯を消さない。同じgroup/channel/pitchの再Note Onは次のNote Off
まで点灯する。エンジン停止後は空表示とし、ノード破棄時にslotを空にするので削除されたインスタンスの値は表示しない。

リリース音やサステインの残響を表すメーターではない。MIDI 2のvelocity-zeroは
既存UAPMDノート追跡と同じ解除扱い。フレーム間にOn/Off両方が来る短音は見えない場合がある。
保存形式・復元経路は変更せず、移行対象データは生じない。

## オシロスコープの参照仕様

参照元は兄弟repo cat-plugin-player の `24775301b6d9f367c88c93583e4c0bf7d91cfb97`。
`scope_stream.rs` / `scope_boundary.rs` / `scope_drift.rs` / `scope_trigger.rs` と
その回帰テストを同名で移植した。入力配送・出力取得は本アプリのnative bridgeへ接続する。

| 参照元の挙動・根拠 | 移植先・検証 |
|---|---|
| `scope_stream.rs`: MIDI音高から完全窓を作る。既定4周期 | 同名moduleとstream回帰テスト |
| `scope_boundary.rs` / `scope_drift.rs`: 山頂の位相追跡、note境界合わせ、500ms追従 | 同名moduleとboundary/drift回帰テスト |
| `scope_capture.rs`: 有界SPSC、欠落検出、note-off後もtailの音高を保持 | 同名module。UMPはUAPMDに合わせ絶対sample offset、MIDI 1/2とmessage長を扱う |
| `scope.rs`: 音声threadから分離して全完成窓を解析し、最新窓を公開 | 同名worker。scope E2Eで実機の切替・停止再開を検証 |
| `scope_ui.rs`: 暗い背景、緑L/青R、共通peak normalize、平行移動とclip | 同名UIとnormalize回帰テスト。GUI全体は既存の16ms再描画を利用 |

簡単な表示を優先する指定に合わせ、対象は準備済み音源1つに限定する。
右paneの鍵盤直下に高さ100で固定する。表示確認後の指定により、
4周期/Similarity/guideなしで固定し、設定menu・設定保持・切替処理は持たない。
Zero crossや他の周期数の処理も削除した。
参照元のspectrum/Lissajous、配置切替、ラベル表示・専用shortcutは移植しない。
参照元で永続化されるのは解析表示の配置であり、本アプリは固定配置のため保存項目を増やさない。
既存設定・snippet・音色・起動復元経路は変更しない。

音高は入力MIDIから得るもので、音声からの基本周波数推定ではない。
多声音・pitch bend・公称音高と異なる音には参照元と同じ制約がある。
表示窓の共通peak正規化によりL/R比は保つが、窓間の絶対音量は表さない。

## 回帰検証

このdirectoryの `note-monitor.js` を E2E 入口（`--e2e note-monitor`）が disk から読んで cell ごとに流す。
観測は `repl.test.notes()`（bridge の `notes` command と同じ `held_notes`）。

共通周期loopの時刻検証では、同じ入力点にある有界traceを明示的に有効にする。
各instanceのsample frame、Note On/Offとrender leadをcontrol threadから読み、
音声threadではatomic配列への記録だけを行う。表示用の最新状態とは独立している。
通常GUIではtraceを有効にしない。詳細と回帰は
[loop-playback](../loop-playback/README.md) にある。

```powershell
python scripts/build.py
cargo fmt --check
cargo clippy --release -- -D warnings
cargo test --release
python scripts/e2e.py note-monitor timeline cold stop-run
```

波形の追加検証は `python scripts/e2e.py scope`。実機で60→72への音高切替、
表示窓の公称4周期長、非無音の有限sample、Stop後の67での再開を検証する。
描画の滑らかさ・外観一致は自動E2Eだけでは判定しない。
実GUIで演奏中の4周期波形と配置を確認し、userの表示合格後に設定処理を除去した。

`note-monitor` は実機Surgeで、停止中キューと実際の配送の区別、和音、範囲外72、
2台目の音源（Six Sines）との和集合と片方だけの解除、
例外後の入力、Stop/restart、通常サンプルのNote On/Off、MIDI 1/2、チャネル別解除、
velocity-zero、CC120/123を検証する。他の3ケースは既存再生動作の回帰検証。
画面では60/61/64同時点灯、Stop消灯、標準サンプルの60点灯・消灯、
スクロール時の鍵盤固定を確認した。既存キャッシュ上のWindows Releaseビルドで検証済み。
新規環境での全依存取得は今回の検証範囲に含めない。

## 適用失敗・更新・撤去

`apply.cmake` は配送点・メンバー・出力処理の戻り位置・graphソースの前提が欠けた場合、補修名、対象、
失敗条件、上流revision、本書の場所を出力して停止する。失敗時は上流変更と
`processAudio` / `prepareEventInput` / stop flushの経路を読み、観測位置を再確認する。
アンカーの検査を外してビルドを通さない。上流に公式入力観測APIが追加済みか調べてから
更新し、代替APIで同じ回帰検証が通れば生成コピーと本観測処理を撤去する。

作者への報告状態: **未報告**。提案文は `upstream-report.md`。外部送信はしていない。
