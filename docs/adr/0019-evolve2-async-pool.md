# ADR 0019: evolveは、非同期の要求と音源の予備poolを採る

- 状態: evolve の土台として採用、現行 evolve は削除
- 決定者: user（現行evolveを置き換えず、別のJSとRust DLLでcmrtと同じ構造のevolve2を新しく作る。要求はworkerへのsubmitだけで即座に返し、決着はpollで回収する。cycle変更・取り消しで世代を進め、古い世代の完了は適用しない。論理slotと物理instanceを分け、同じpluginへの切り替えはstateの再適用だけ、別pluginへの切り替えは種別ごとの予備の差し替えで行う。状態機械・台帳・pool・worker・世代・時計・fadeはRust、C++は生成・破棄・state適用・graphへの着脱の薄いwrapに限る。最後に現行evolveと同じ条件で比べる）。比較の後、userはevolve2をevolveの土台にし、現行evolveを消して名前をevolveに統一することを決めた。物理instanceを1pluginだけ載せた未公開graphとすること、Host APIを完了callback型にすること、予備の数、UAPMDを呼ぶthread、evolveとの排他はAIの提案をuserが承認
- 関連: [ADR 0017](0017-evolving-loop-prototypes.md)、[ADR 0020](0020-headless-clap-host-for-evolve2.md)、[ADR 0027](0027-evolve-groups-injected-from-js.md)

## 決定と理由

[ADR 0017](0017-evolving-loop-prototypes.md)の現行evolveは、UI/control threadから同期にpluginを生成し、切り替えのたびに旧instanceを破棄してから新しいinstanceを作る。予備poolはA/Bの2個という決定と両立しないため採らなかった。userはこれを置き換えず、clap-mml-render-tuiのgrid sequencerとclap-mml-play-serverが持つ構造（preloadの世代、論理slotから物理instanceへの台帳、種別ごとの予備の袋）を同じ形で持つevolve2を別に作り、構造の差だけを比べることを選んだ。evolve2はこの比較のために、ADR 0017が却下した予備poolを採る。比較の結果（下の「結果と制限」）からuserはこの構造をevolveの土台に選び、以下の`evolve`はこの構造を指す。

- **物理instanceは「1pluginだけを載せた未公開graph」とする。** slotへの着脱はevolveの`Source`が返すgraphを差し替えるだけで、予備はgraphを持つが返さないのでDSPが呼ばれない。同時に処理されるのは現在音と次の音色の最大2個で、ADR 0017の上限を保つ。
- **予備は、要求されたことのある種別ごとに1個とし、追い出さない。** 種別ごとの物理数は「slot数 + 予備の目標数」を超えない。CLAPは同種のinstanceが0個になるとmoduleを外すので、予備はmoduleの再ロードも避ける。1個は仮の値である。
- **UAPMDの生成・state読込・破棄は、全部mainのtaskとして積む。** RustのworkerからUAPMDを直接呼ばない。CLAPはUI-thread要件が全非audio操作なので、別threadから呼んでも生成・state読込・破棄はmainへ回され、mainの停止時間は変わらなかった。直接呼ぶとlockの無い`RemidyAudioPluginHost::instances`やprocess全体の`current_path`にmain以外から触る。Host APIは要求を積んで即座に返り、完了をRustのcallbackで受ける。
- **evolveを閉じるときは、Rustの`destroy`を別threadで呼び、mainは積まれたtaskを処理しながら待つ。** `destroy`は生成中の要求と捨てた読込の完了を待つが、その完了はmainのtaskから来るので、mainで同期に呼ぶと戻らない。
- **JSは決着を待つloopを持たない。** UAPMDの処理はmainのtaskで走り、GUIでは毎frameの`poll`がそれを処理するので、JSが待つとUIを占有し非同期の意味が消える。このため再生開始から最初の音色が準備できるまでは無音になる。

## 結果と制限

同じ固定stateの比較で、同じpluginへの切り替えは現行の生成約43msに対し、stateの再適用で約0.8msになった。別pluginでも予備の差し替えでSurge XT・TyrellN6は約1ms、Vaporizer2は約12ms（現行は約180msでUIを止める）になった。

固定stateの比較でUIを最も長く止めていたのは、生成でなくSix Sinesのstate読込だった。止めていたのはplugin自体でなく、UAPMDのCLAP hostがpluginからのrescanのたびに全parameterを読み直す処理で、evolveのinstanceにだけそれをしないhostを付けて、固定stateでのSix Sinesの切り替えのUI停止は約1.6秒（新しいinstanceでは約4.9秒）から最大約51msになった（[ADR 0020](0020-headless-clap-host-for-evolve2.md)）。

固定stateの比較はcatalogを通らない。catalogから音色を選ぶ`try-evolving-sequencer`の流れでは、切り替えのたびに`repl.patchCatalog`がcatalogを読み直して全patchを分類し、UIを約1.5秒止めていた。分類済みの一覧をprocess内に1回だけ作って持つようにした後の`try-evolving-sequencer`の35秒の計測では、切り替えのUI停止はplugin自身（Surge XT・Six Sines）の約130〜190msだった。

evolveは、現行evolveが持っていた1coreの稼働維持とTIME_CRITICAL（[ADR 0017](0017-evolving-loop-prototypes.md)）を引き継がず、現行evolveと一緒に消した。現行evolveがあった時点でも、この構造の演奏中に引き上げられたthreadは0個だった。固定stateの4pluginの90秒で、workerの破棄・期限超過は消す前後とも1回あたり約20〜40回あり、TyrellN6とSix Sinesが鳴る区間に固まる。1coreの稼働維持は、[ADR 0023](0023-evolve-keeps-one-core-busy.md)でevolveのRust DLLに戻した。
