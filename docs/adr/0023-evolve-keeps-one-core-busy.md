# ADR 0023: evolve の演奏中は、Rust DLL の1本の thread で1 core を稼働させる

- 状態: 採用
- 決定者: user（変化する loop で TyrellN6・Six Sines が鳴る区間の期限超過を減らし、他の音色の区間と UI の止まりを悪くしないこと。原因を測ってから直し方を決める進め方）。1本の thread を回す方式、範囲を evolve の演奏中に限ること、thread priority を変えないことは AI の設計
- 関連: [ADR 0017](0017-evolving-loop-prototypes.md)、[ADR 0018](0018-buffer-snippet-without-ftz-daz.md)、[ADR 0019](0019-evolve2-async-pool.md)

## 決定と理由

evolve の Rust DLL（`evolve-dll`）に1本の thread を持たせ、audio callback の `begin` が最後に来てから 100ms 以内のあいだは `spin_loop` で回し続け、それ以外は 1ms ずつ眠らせる。evolve を閉じると止めて join する。代わりに演奏中は約1 core 分の CPU を使う。

期限超過の原因は plugin の平均の重さでも worker の割り振りでもなく、電源設定「最小のプロセッサの状態」が低い（5%）ときに、block の処理時間がときどき3〜4倍に伸びることだった。Six Sines（固定 state の音色）の1 instance の1 block は、中央値が 5% と 100% で同じ約 6.8ms のまま、p99 が 5% では約 20.8ms、100% では約 8.5ms だった。worker を使わず callback thread だけで処理しても同じように伸びた。別 process で1 core を回し続けるだけで、5% のまま 100% にしたときとほぼ同じ結果になった。

- **thread は1本で、priority は変えない。** 通常の優先度の1本の spin で、5% のまま固定 state の TyrellN6・Six Sines の区間の期限超過が 0 になった。
- **pulse は `begin` の中の atomic の store だけにする。** audio thread で alloc もしないし、待ちもしない。
- **回すのは evolve の演奏中だけ。** 通常の track の再生には入れない。callback が途絶えると、100ms で回るのをやめる。
- **thread を作れなければ、evolve を開かない。** spin 無しで黙って動くと、期限超過が戻ったときに理由が見えない。

## 却下した案

- **app が電源設定を変える。** [ADR 0018](0018-buffer-snippet-without-ftz-daz.md) で user が「アプリによる電源設定の自動変更は追加しない」と決めている。
- **UAPMD の worker の待ち方（50µs の sleep）を spin に変える補修。** 依存への補修になり、worker の数だけ core を使う。1本で足りた。
- **次の音として用意した slot を、fade の直前まで処理しない。** 期限超過のうち、gain 0 で処理されている Six Sines の分しか減らない。

## 結果と制限

固定 state（4種、6 worker、90秒、最小のプロセッサの状態 5%）の3回で、期限超過は直す前の 15〜40回から 0〜1回になった。TyrellN6・Six Sines が鳴る区間の期限超過は3回とも 0 だった。

user の流れ（`try-evolving-sequencer`、catalog から音色をランダムに選ぶ、180秒×9回）では、Six Sines・TyrellN6 の外の区間の期限超過は直す前の 238件（13.9件/分）から 9件（0.4件/分）になった。TyrellN6・Six Sines の区間は、9回のうち8回で計 1件だった。残る1回は、重い Six Sines の音色に当たって 1428件だった。

Six Sines には、spin があっても期限に収まらない音色がある。固定 state で5つの音色を測った（20秒、2 slot とも同じ音色、6 worker）。期限超過は spin なしで 32〜228回だった。spin ありでは3つが 0 回、「Hope Evolution」が 71回になった。「Keys/S+H Pad OB8 + CZ」は spin ありでも 1 instance の中央値が約 17ms、p99 が約 23ms で、期限（約 21.3ms）を超える。期限超過は 190回から 179回にしか減らない。この音色の分は plugin 自体の重さで、spin では直らない。

演奏中は process の CPU が約1 core 分（約 90〜100 ポイント）増える。user の流れで出た約5秒の UI の止まりは、Vaporizer2 の音色「PD Pulsation.vvp」の読込の中で起きていた。固定 state で同じ音色を読ませると、spin の有無に関わらず読込のたびに約5秒止まった（spin あり 5032・5120ms、spin なし 5055〜5130ms）。
