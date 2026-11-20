# ADR 0024: 「最小のプロセッサの状態」を100%にするsnippetを置き、正常終了時にsnippetが指定した値へ戻す

- 状態: 採用
- 決定者: user（UX検証の結果、100%のsnippetでのUX検証が必要と判断した。100%にするときは、正常終了しなければ100%のまま残ることを通知し、終了時には自動で5%に戻したことを通知する。通知は egui の modal にする。どの文言をいつ出すか・egui の呼び出し・電源設定の組み立ては JS の lib に置き、Rust は薄い API だけを持つ）。戻す値を snippet が渡すこと、AC・DC の両方を書くこと、電源 API を直接呼ぶこと、終了時の処理を JS の `repl.app.onExit` で持つことは AI の設計
- 関連: [ADR 0018](0018-buffer-snippet-without-ftz-daz.md)、[ADR 0023](0023-evolve-keeps-one-core-busy.md)

## 決定と理由

Rust は薄い API を3つだけ持つ。`repl.power.setMinProcessorState(percent)` は有効な電源プランの「最小のプロセッサの状態」を AC・DC とも書く。`repl.ui.dialog(title, text)` は OK だけの egui modal を出す。`repl.app.onExit(name, fn)` は正常終了の時に `fn` を1回呼ぶ（JS 側の表で、Rust は閉じる要求と終了処理で `__replRunExitHandlers()` を呼ぶだけ）。何を書き、いつ戻し、何を知らせるかは `snippets/builtin/lib/power.js` が組み立て、組込み snippet `set-min-processor-state` はそれで 100% にして 5% へ戻す。ADR 0018 で user が想定した「戻さずに終わる経路で設定が残る」リスクは、user の判断で引き受け、100% にしたときの modal で知らせる。

- **戻す値は変更前に読まず、snippet が渡す。** 異常終了の後に読むと、残った 100% を元の値として控え、終了時に 100% へ戻してしまう。snippet の値なら、再起動後に `percent` に 5 を指定して選ぶだけで戻る（`percent` と `restorePercent` が同じときは終了時の処理を外し、通知もしない）。
- **GUI では最初の閉じる要求を1回だけ保留する。** 終了時の処理が出した modal を OK で閉じ終えてから窓を閉じる。modal に取消しは無い。強制終了・クラッシュでは戻らない。
- **`powercfg` の子 process でなく、powrprof の API を呼ぶ。** `repl.*` は UI thread で同期に走るので、子 process の待ちを置かない。

## 却下した案

- **ADR 0023 の1 core を回す thread を、通常の再生にも広げる。** 電源設定に触らず、設定が残るリスクも無い。user は 100% そのものでの UX 検証を選んだ。
- **終了時に、変更前に読んだ値へ戻す。** 上の理由で、異常終了の後に 100% から抜けられなくなる。
- **戻す値と通知文を Rust が持つ。** 文言と戻す判断が Rust に入り、snippet から変えられない。user の方針（Rust は薄い API）に反する。

## 結果と制限

強制終了・クラッシュの後は 100% のまま残る。CLI・automation では `repl.ui.dialog` が使えないので、この snippet は設定を書いた後に例外になる（戻す処理は登録され、終了時に戻す）。AC・DC の両方を同じ値で書くので、元の AC と DC が違った場合（たとえば AC 100%、DC 5%）は、終了後に両方とも `restorePercent` になる。
