# 共通周期の MIDI loop

## 原因と適用

UAPMD の TimelineFacadeAudio は周回境界でブロックを分割するが、公開 JS API には
loop 設定と準備済み音列の境界公開がない。UI 時刻で remove/add すると、先読み済みの
音声と複数 track の latency が違うため途中の周回へ変更が混入する。

対象 revision は `4ec5c3b1c30130fdd3b7d4b375082273247e561f`。
`apply.py` は `TimelineFacadeAudio.cpp` の callback 開始、位置変換・segment 分割、
既存 render segment の終わりに app 所有の処理を接続する。
音列だけでなく周期も変わるため、transport と既存 clip にも同じ位相と分割を使う。
生成先は `target/shim/loop-playback`。
依存 checkout は変更せず、通常の `python scripts/build.py` が再 clone / 既存 cache の
どちらでも生成する。元ソースと変換 script の変更は再構成で反映する。

## script と native の境界

`repl.timeline.configureLoop(beats, bpm)` は停止中に共通周期を sample へ丸める。
同じ設定なら演奏中も変更しない。異なる設定の演奏中 configure は拒否する。
`publishLoopClips([{trackIndex, filepath}, ...], beats, bpm)` は新周期と完成した全パートを
一式として公開する。演奏中は固定 tempo のまま周期を伸長・短縮できる。
beats を省略する従来の固定周期呼出しも使用できるが、自動追従は script が明示的に渡す。
SMF 読込と sample 時刻変換は既存 MidiClipReader / MidiClipSourceNode を使う。
全パートの検査が終わるまで現在の plan・周期を変更しない。group 0 の Note On/Off、
固定共通 tempo、最大 32 track、各 16384 event を受け付ける。
同じ track の複数音列は script 側で合成する。音色変更の message は拒否する。

組立例は配布される `try-loop-mml` snippet。通常起動後にその snippet を実行し、
`cegb` を編集して再実行する。同名 `main` は script の map 内で置き換わる。
MML を `cegbdf` にすれば3拍、`crr` なら末尾の休符も含む1.5拍になる。
loop を鳴らす snippet は `include('builtin/lib/loop.js')` から共通関数を読み込む。
`prepareLoopPart(trackIndex, input)` は別々の演奏用SMFと長さを準備し、
SMF 保存は `lib/midi.js` と共有する。lib の読み込み自体では再生状態を変えない。
`loopPhraseBeats` は cat-smf-editor の phrase_end と同じ parser → AST → channel末尾の
非発音program marker → pass3 の時計を使う。480tick を1拍へ換算し、和音を逐次加算しない。
そのための既存関数だけを薄く公開し、marker配置・最大時刻・拍換算は script に残す。
marker は計算用ASTだけに置き、演奏用SMFやpluginへ渡さない。音長の独自parserはない。

別パートは別 track を用い、snippet の関数を次のように組み立てる。

```js
var next = Object.assign({}, loopParts);
next.second = prepareLoopPart(otherTrackIndex, 'drr');
playLoopParts(next, 120);
```

`playLoopParts` は tutorial の `play()` と対になる入口で、loop だけが鳴る状態にする。
全 track に通常 clip があるか loop が未確立なら、再生と音声エンジンを止め、確立済みの loop を
`clearLoop()` で破棄し、全 track の通常 clip を消してから公開する。通常 clip が無く loop が
確立済みなら停止せず、下の境界交換で更新する。公開後は各パートの track の instrument（先頭
node）だけ bypass を解除し、音声エンジンを有効にして、止まっていれば再生を始める。
`publishLoopParts` はその下請けで、通常 clip に触れない。
`publishLoopParts` が最長パートを共通周期に選び、公開成功後だけ `loopParts` を交換する。
短いパートは先頭から一度演奏して残りを休み、独立反復・伸縮・LCMを使わない。
ファイル生成、パート名、map、長さの決定、再生開始は script 所有で、native は抱き合わせない。
確立済み loop では、正の長さにならないMML、共通tempoと異なるSMF、読込失敗を
現在の演奏を保って拒否する。

通常再生からの初回導入では、loop の sample rate がまだ未設定のため、準備後の初回公開前に
script が再生と音声エンジンを停止する。既存のエンジン再開処理で Stop flush を消化し、
新しい先頭ノートが flush に消されることを防ぐ。公開成功後だけ `loopParts` を設定し、snippet・lib を再読み込みしても
以後の更新では停止しない。初回の native 公開自体が失敗した場合は停止状態を保つ。
native の演奏中 timing 変更の拒否条件は維持する。

native は既存 snapshot publisher の 4 reader slot を用い、active / 境界交換待ち / 新規公開と、
audible playhead がまだ参照する直前の plan を保持する。
音声 thread では確保、破棄、ファイル I/O、JS、待機 lock を行わない。
要求を最初に観測した callback で全 track の render lead と直前に生成した最大終端を調べ、
まだ render していない最初の共通境界を選ぶ。既に一部 track が交換を render した場合は
その交換を全 track へ渡してから最新要求を受け付ける。
十分早い要求は次の周回、既に render 済みの境界に間に合わない要求は次の反映可能境界になる。
plan は丸め済みの新周期を保持し、交換境界を新しい位相0にする。たとえば4→3拍でも
raw位置を3拍で単純に割った余りから始めない。新しい起点から整数sample周期を反復する。
未開始の予約は最新要求へ置換できるが、一部trackがrenderした予約は混ぜずに全trackへ渡す。
audible位置、raw render位置、track/master lead、直前に生成した最大終端を区別し、
各segmentの位置・長さとtransportのloop範囲を対応する旧新planに揃える。
`loopStatus()` の boundary は raw render sample、revision は観測した境界交換の版である。
revision は初回の plan 採用や配送経路によっては0のままなので、再生成功の判定には使わない。
周期は transport、発音は実ノート配送で確認する。
audible playhead と混同しない。dropped はイベント容量超過の累積値。

右 pane の読取専用ピアノロールは、通常 clip の公開 API データに、app 内部の `timeline-view`
読取で得る安定した track ID・source の sample 時刻・master tempo/signature・loop plan を合わせる。
loop は公開直後の最新要求ではなく、audible playhead に対応する plan と位相を表示する。
表示用 pointer は音声側の guard で保護し、解放前に無効化する。確保・回収・JSON 化は
control thread に限定し、plan の交換中は直前の画面を維持する。
起動直後の自動生成 track の ID を記録し、未使用のものだけを隠す。
user が追加した空 track は表示し、削除で index が詰まっても ID で区別する。
この表示情報は保存形式や起動復元を変更しない。

終端を含まない既存 segment の意味を守り、次周先頭でこの機能の保持ノートだけを終了してから
新しい Note On を渡す。終端ちょうどの Off と周期を跨ぐ長音もここで終了する。
負のrender位置を使うprerollでは、この音列の開始前に先頭音を繰り返さず、
時刻0に到達したブロック内offsetから配送する。
Stop は既存 transport 拡張から予約を破棄する。プラグイン再初期化や毎周 jump/start は行わない。
transport Stop に続く audio engine 停止で再度 Stop が届いても、採用済み plan は保持する。
未採用の新規公開は取り消し、予約の取り消しと採用済み音列の消去を混同しない。
停止中の `repl.timeline.clearLoop()` は plan と周期を破棄し、transport の loop 範囲も解除する（tutorial の `reset()` が呼ぶ）。
loop は実行 session 内だけに保持し、保存形式、プラグイン状態、起動復元は変更しない。

## 検証と診断

`python scripts/e2e.py loop loop-update` は通常の Ready / lib / Stop の入口を使う。
入口 script はテスト専用で、配布 snippet の内容に依存しない。期待する音高・発音位置・周期は
MML 変換器から求め、音高・音長の意味は MML ライブラリ側の検証に任せる。
`python scripts/loop-entry-test.py` は各 loop snippet を新しい session で検証し、
通常再生からの初回導入で1回だけ停止すること、再実行・更新・不正更新で追加停止しないこと、
既存clipと公開済みmapを保全することを確認する。
`python scripts/e2e.py loop-replaces-clips` は通常再生の後に `playLoopParts` を呼び、
plugin の無い track を含む全 track の通常 clip が消え、以後の Note On が loop の音高だけになることを確認する。
`loop.js`・`loop-update.js`（`--e2e` の case。build 不要）の先頭も通常再生中から開始し、最初のノートを含む順序・sample間隔・master信号を検証する。
既存 note-monitor に、検証時だけ有効な 16384 件のプラグイン入力 trace を接続する。
sample frame と Note On/Off を atomic 配列へ記録し、control thread が読む。
容量超過は検証失敗とする。master 出力の信号検証も併用する。
2台のSurgeと32-sample latencyのSurge XT Effectsを使い、入力時刻にrender leadを加えた
共通時刻で位相と更新を比較する。音色・effect状態の同一性と既存clip保持も検証する。
native_regression.hppではproductionの選択・配送処理を使い、境界直前、先読み差、
交換途中の再要求、跨ぐ長音、同音再発音、負のpreroll、Stopの具体例を決定的に検証する。
周期1000→1500→750sampleの非倍数起点、伸長後半、block途中の交換、端数周期の反復も検証する。
表示についても、render head が先行する間の旧 plan 保持、audible 境界での音列・位相交換、
Stop で予約が取り消された後の表示を検証する。
`python scripts/e2e.py timeline-view` は未使用 track の非表示、明示的な空 track の保持と reindex、
開始位置を持つ和音 clip、通常 clip と loop の共存、停止・clear 後の表示を実モデルで確認する。
`loop-update.js` は2→3→1.5拍、休符、不正入力時のmap・音列・周期保全、
先読み差のある2trackの最長選択と短縮、transport範囲を実配送とmaster信号で確認する。
trace は Program Change も別のatomic counterで数え、検出markerの不配送を検証する。
既存の `timeline note-monitor mml-clip chord-clip snippets partial-run stop-run` も実行する。
Rust の fmt、clippy、test、release build と native を含む build が必要。
`python patches/uapmd/loop-playback/test_apply.py` は生成anchorと拒否条件を検証する。

## 上流変更と撤去

適用前に唯一の anchor を検査する。失敗時は補修名、対象、条件、上流 revision、
本書の場所を表示して停止する。target source 一覧の不一致でも停止する。
該当する callback / segment の上流変更を調べ、検査を迂回しない。
上流が安全な loop 設定と render 境界公開を提供したら、先読み・複数 track・Stop の
同じ検証で置換できることを確認して shim を撤去する。

作者への報告状態: 未報告。`upstream-report.md` は提案文で、外部送信していない。
