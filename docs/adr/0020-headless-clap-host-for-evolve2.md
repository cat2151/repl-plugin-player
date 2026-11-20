# ADR 0020: evolveのinstanceにだけ、parameterを総なめしないCLAP hostを付ける

- 状態: 採用
- 決定者: user（UAPMDのCLAP hostを一律に直さず、evolve2が作るinstanceにだけ別のhostを付けること。置き場所をclap-note-dialectの生成物の後段に足す新しい補修にすること。範囲をevolve2に限り、現行evolveの`evolution.load`で読むinstanceは今のhostのまま残すこと）。host名`HeadlessCLAPHost`はAIの提案をuserが承認。印を`thread_local`の深さで渡すこと、変える関数を`paramsRescan`だけにすることはAIの設計
- 関連: [ADR 0019](0019-evolve2-async-pool.md)、[ADR 0022](0022-parameter-value-sweeps.md)（trackのinstanceのVALUESのrescan）、[補修の説明書](../../patches/uapmd/headless-clap-host/README.md)

## 決定と理由

evolveでSix Sinesへ切り替えるたびにUIが止まっていた原因は、Six Sinesの`stateLoad`ではなく（自分の値を書き換えてすぐ戻る）、UAPMDの`RemidyCLAPHost::paramsRescan`だった。pluginからrescanを受けるたびに全parameterの値を読み直し、初回は一覧も作る。Six Sines（2554 parameter）はstate読込1回で同期に1回、`request_callback`経由の`onMainThread`でもう1回rescanするので、同じinstanceへの読み直しで約1.6秒、新しいinstanceで約4.9秒UIが止まった。

- **総なめを省けるのは、値を見ている者がいないinstanceだけとする。** evolveのinstanceは未公開graphに載り、trackのUI・`repl.ui.show`・scriptのどこにも出ないので、rescanの結果を読む者がいない。名前`HeadlessCLAPHost`はこの根拠（画面にも出ない）を表し、速さはその結果である。
- **hostの種類は、生成の間だけ有効な`thread_local`の深さで渡す。** UAPMDはinstanceごとにhostを1個作るので、作る瞬間に決めれば後から来るrescanも全部止まる。引数で渡すと、`addPluginToPreparedTrack`からhost生成までのUAPMDの関数の署名を何段も変えることになる。evolveの生成はmainのtaskの中で同期に走るので印が届き、もし別threadへ移っても今のhostに落ちるだけで壊れない。
- **`HeadlessCLAPHost`が変えるのは`paramsRescan`だけで、全flagで何もしない。** evolveのinstanceの一覧はrescanの内側からしか作られないので、INFO・ALLで作り直す対象も無い。`requestCallback`はpluginが`onMainThread`で他の仕事をしうるので残し、`paramsRequestFlush`は音に要るので残す。

## 却下した案

- **`RemidyCLAPHost`を一律に直す（一覧がまだ作られていないinstanceではVALUESのrescanを何もしない）。** 既存のtrackのinstanceの挙動、UIのparameter表示の更新まで変わる。userは範囲をevolve2に限ることを選んだ。
- **UAPMDの`loadState`を「stateを流すだけ」と「値を取り直して通知する」に分け、evolveが前者を呼ぶ。** `onMainThread`から来る2回目のrescanを止めるには、instanceに「総なめしない」状態も別に持たせる必要があり、hostを替えるより手が多い。
- **VALUESのrescanは印を付けるだけにし、総なめを別の関数に切り出す。** 既存のtrack向けに、誰がいつ総なめを呼ぶかを決め直すことになる。

## 結果と制限

固定stateで測ったSix Sinesのevolveの切り替え（新しいinstance・予備との差し替え・同じinstanceへの読み直し）で、evolveのinstanceへの`paramsGetValue`・`paramsValueToText`は0回になり、UIの停止は最大約51ms（frame monitorの閾値は約81ms）になった。catalogから音色を選ぶ`try-evolving-sequencer`の流れでは、Six Sinesへの切り替えのUI停止は約130msで、その間UI threadはSix Sines自身の中にいた。

evolveのinstanceをUIやscriptに出すようにしたら、parameterの一覧と値が更新されないので、このhostを外す必要がある。
