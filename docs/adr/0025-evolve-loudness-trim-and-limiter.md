# ADR 0025: evolve の音量は、slot ごとに聴こえる前に測って固定し、limiter で割れを防ぐ

- 状態: 採用（目標 -18 LUFS・補正の上限 ±18dB・ceiling -1 dBFS・release 200ms は仮の値）
- 決定者: user（evolve の音量が plugin と音色でまちまちなので、アプリのどこかの層で autoGain を入れる。補正 gain は prime で測った値に固定し、鳴っている間は追わない。「音が大きすぎて割れているのか、本当に割れた音なのか区別がつかない」ので、limiter でピークも保護する。演奏開始の無音で混乱するので、最初の slot は測りながら鳴らす）。置き場所を evolve-dll の `Playback` にすること、K-weighting で測ること、測る区間を最初の note-on から和音1つ分にすること、最初の slot だけ和音の途中から鳴らすこと、その補正を1秒かけて移すこと、limiter を crossfade の前で slot ごとに掛けることは AI の設計
- 関連: [ADR 0017](0017-evolving-loop-prototypes.md)、[ADR 0019](0019-evolve2-async-pool.md)

## 決定と理由

evolve の各 slot の出力を、`evolve-dll` の `Playback::mix` で「測定（K-weighting、ITU-R BS.1770・gate なし）→ trim → limiter → crossfade の gain」の順に処理する。次の音色の slot は、もともと gain 0 で和音1つ分鳴らしてから fade に入る。その区間で生の出力を測れば、聴こえる前に trim が決まる。

`try-evolving-sequencer` の流れで測ると、補正前の loudness は -7.7〜-31.8 LUFS（24dB）ばらつき、peak が +0.8 dBFS・+1.35 dBFS の音色もあった。trim を入れた後は、1 slot だけが鳴っている区間の master の RMS のばらつきが 9.8dB だった（同じ回の補正前の測定値では 20.8dB）。

- **測る区間は、その slot の最初の note-on から和音1つ分にする。** engage から数えると、和音の境界を待つ無音が区間に入り、測れる音がほぼ 0 になることがある。
- **最初の slot は、和音の頭を待たずに進行中の和音から、測りながら補正なしで鳴らし、trim が決まったら1秒かけて trim へ移す。** 和音の頭を待って測り終わるまで gain 0 にすると、演奏開始から約 6.7 秒無音になり、user が混乱した。直すと、聴こえ始めは約 0.2〜0.5 秒になった。一度に trim を掛けると、音量が段になって変わる。hold は最初の slot が鳴り始めてから数える。
- **次の slot は、今までどおり次の和音の頭から測る。** 同じ instance に state を読み直した直後の block の note-on を plugin が落とすことがあり、保持された和音は打ち直されないので、和音の途中から始めると測定中ずっと無音になる（Surge XT で、1和音を保持する plan のとき）。
- **limiter は slot ごとに、crossfade の gain の前で掛ける。** channel を連動させ、attack は即時、lookahead は無い。各 slot が ceiling 以内なら、和が 1 の crossfade でも A と B の合計は ceiling を超えない。lookahead を入れないので latency は増えない。そのため、attack の1 sample だけは音が歪む。
- **`mix` は slot の全 channel を1回で受け取る（ABI version 2）。** channel を連動させるのと、channel ごとに filter の履歴を持つため。
- **目標の音量は -18 LUFS。** 上の流れで測った音色の中央値（約 -17 LUFS）に近く、補正を入れても全体の音量は下がらない。

## 却下した案

- **最初の slot も、測り終わるまで gain 0 にする。** 段は出ないが、上の無音が出る。

- **JS が master の meter を見て `track.setVolume` で追う。** JS に音声のリアルタイム処理を持たせないという AGENTS.md の規則に反する。UI thread の周期に縛られ、音量の変化が遅れて聴こえる。
- **catalog に、patch ごとにオフラインで測った音量を持つ。** 正確だが、patch の数だけオフラインで render する必要があり、規模が段違いに大きい。
- **master に compressor/limiter の plugin を挿す。** 小さい音色は持ち上がらず、音そのものも変わる。
- **鳴っている間も、数秒の時定数で trim を追う。** user が固定を選んだ。音量がうねるおそれがあり、調整する定数も増える。

## 結果と制限

- 最初の和音の間は、音色ごとの補正前の音量（上の 24dB のばらつき）がそのまま聴こえ、続く1秒で目標の音量へ移る。

- 立ち上がりに鋭い peak がある音色は、trim の後に limiter が押さえ続けるため、目標より小さく聴こえる。上の流れでは、peak と loudness の差が 25.6dB ある Vaporizer2 の音色（補正前 -33.2 LUFS）に +15dB の trim が掛かった。limiter が割れを防いだ代わりに、ほかの音色より約 7dB 小さく聴こえた。
- state を読んだ後に音を非同期で用意する音源は、最初の和音を小さく測られ、上限（+18dB）まで大きく鳴ることがある。Floe（Celtic Harp）では、同じ patch が演奏中の読み直しで 5dB 小さく測られた。割れは limiter が防ぐ。
- master の手前では、evolve の slot だけを ceiling 以内に収める。通常の track と合わせた和は保証しない。
