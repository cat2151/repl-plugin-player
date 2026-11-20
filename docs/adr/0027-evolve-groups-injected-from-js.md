# ADR 0027: evolve は、JS が `evolve.open` で渡す group 構成で動き、track と slot の対応は Rust が持つ

- 状態: 採用（group は track 2本に限ること・slot を group をまたいだ通し番号にすること・hold と fade を全 group で共有すること・`timeline` の group が和音の境目を待たずに fade することは仮の決め）
- 決定者: user（`try-evolving-sequencer` の scatter の音色の切り替えを pad と同じ evolve-dll の非同期の仕組みに載せる。Evolve は1つのまま slot の組を足し、Evolve を2つ持つ案は採らない。track 数・track 構成は Rust の build 無しで任意にし、JS が構成を Rust へ注入する。C++ には UAPMD との接続で C++ でなければ書けない最低限だけを置く。scatter には音量の補正を掛けない。`evolve.open` の `groups` の形、`input` を `chord` と `timeline` の2種にすること、scatter も track 2本で crossfade すること、測定を `chord` の group だけに持たせること、pad も新しい形へ移して旧い形を残さないこと、C++ と Rust の境界は AI の提案を user が承認）。slot の番号の付け方、group ごとに持つ物と全体で1つの物、期限超過の blame を全 group で1件にすること、`timeline` の group が切り替える時刻、ABI の形は AI の設計
- 関連: [ADR 0019](0019-evolve2-async-pool.md)、[ADR 0025](0025-evolve-loudness-trim-and-limiter.md)、[ADR 0026](0026-evolve-skips-on-deadline-miss.md)

## 決定と理由

scatter の音色の切り替えは、UI thread で同期に plugin を生成して state を読んでいた。scatter の instance は track に載る通常の instance で、[ADR 0020](0020-headless-clap-host-for-evolve2.md) の headless な CLAP host が付かないため、Six Sines の新しい instance への切り替えで UI が約3.5秒止まった。evolve の slot に載せれば、生成・state 読込は予備 pool と headless な host を通る。

- **`evolve.open` は `{ groups: [{ tracks:[a,b], input:"chord" | "timeline" }, ...], holdMs, fadeMs }` を受け、Rust が `create` の時に構成から slot の配列を確保する。** group は「互いに crossfade する track の組」。group を足すのは JS の引数だけで、Rust・C++ の build は要らない。音声 callback の中では確保しない。
- **`input` は event をどこから取るかの違いだけ。** `chord` の group は evolve の和音の時計から note を作って track の event 入力を書き直し、[ADR 0025](0025-evolve-loudness-trim-and-limiter.md) の測定・trim・limiter を通る。`timeline` の group は track が受け取った event をそのまま鳴らし、出力に crossfade の gain だけを掛ける。測定は和音1つ分の窓が前提で、`timeline` の group にはその窓が無い。
- **slot の番号は group をまたいだ通し番号（group i の track j が slot `2i+j`）。** `evolve.submit {slot}`・`evolve.status` の配列・完了の通知の形を、group が1つのときと同じに保てる。
- **group は track 2本に限る。** crossfade と期限超過の扱いが「もう片方の slot」で書かれていて、3本以上の組で誰から誰へ fade するかが決まっていない。group の数は任意。
- **group ごとに持つのは current の slot・hold の経過・fade・印（ADR 0026）、全体で1つなのは 1 core の稼働（[ADR 0023](0023-evolve-keeps-one-core-busy.md)）・loader の worker・予備 pool・世代。** `chord` の group だけが和音の時計と音量の測定を持つ。世代が1つなので、pad の loop を始め直すと scatter の要求中の物も捨てられる。
- **期限超過は block 全体の出来事なので、blame は全 group から1件だけ選ぶ。** [ADR 0026](0026-evolve-skips-on-deadline-miss.md) の順（測定中の slot、fade で入ってくる slot、今の音色に印）を、slot の集合を全 group に広げて当てる。今の音色に印のある group は、候補から外す。
- **`timeline` の group は、次の slot が READY で今の slot の hold が過ぎた block で fade を始める。** scatter の音は和音の時計に無い位置で鳴り、次の slot も同じ clip を受けて鳴っているので、和音の境目に揃える理由が無い。
- **hold と fade は全 group で共有する。** scatter の切り替えの間隔は「JS の乱数の間隔」と「hold + fade」の長い方になり、JS は空いた slot が EMPTY になるまで次の要求を待つ。
- **C++ に残すのは、UAPMD の型か UAPMD の thread でしか触れない物だけ。** plugin の生成・state 読込・破棄、graph の差し替え、`AudioProcessContext` の buffer と event 入力を Rust に渡すこと、track が空かの検査、閉じるときの main queue の pump。track と slot の対応、最小の track で block を始めること、UMP 語の組み立て、`status`・`poll` の JSON、state file の読込と世代は Rust に置く。UAPMD は `beforeTrackProcess` を track 番号の昇順に直列で呼ぶので、Rust は構成の最小の track で block を始める。
- **evolve-dll に JSON の crate を足さない。** `evolve.open` の引数は C++ が choc で読んで構成の配列に写し、`input` の文字列の検査は Rust がする。`status`・`poll` の JSON は数値と少しの文字列だけなので Rust で組む。

## 却下した案

- **Evolve を2つ持ち、scatter 用の Evolve を別に開く。** user が採らなかった。`Source::graph` を差し替える補修が `active` を1個しか持たず、1 core の稼働・worker・予備 pool も二重になる。
- **slot を `{group, slot}` の組で指す。** JS・status・完了の通知の形が全部変わり、通し番号で足りる。
- **`timeline` の group を和音の境目で切り替える。** 上の理由で揃える意味が無い。
- **group ごとに別の hold・fade を持たせる。** `evolve.open` の形を変えるので、使ってから決める。

## 結果と制限

`try-evolving-sequencer` の 180 秒の計測で、scatter の Six Sines の新しい instance への切り替えの UI 停止は 0〜149ms になった（通常の instance のときは約3.5秒）。evolve の slot が2個増え、演奏中の CPU は 1 core の約123% から約145% になった。

Vaporizer2 を選んだ回は、pad でも scatter でも UI が数秒止まることがある。停止中の UI thread は evolve の `poll` から入った Vaporizer2 の中にいて、group 構成では直らない。`timeline` の slot が fade で抜けるとき、evolve はその slot に note-off を送らない。scatter には音量の補正が無いので、音色ごとに音量がばらつく。
