# ADR 0019: 比較用のevolve2で、非同期の要求と音源の予備poolを採る

- 状態: 採用（現行evolveとの比較用。evolveの土台にするかは未決定）
- 決定者: user（現行evolveを置き換えず、別のJSとRust DLLでcmrtと同じ構造のevolve2を新しく作る。要求はworkerへのsubmitだけで即座に返し、決着はpollで回収する。cycle変更・取り消しで世代を進め、古い世代の完了は適用しない。論理slotと物理instanceを分け、同じpluginへの切り替えはstateの再適用だけ、別pluginへの切り替えは種別ごとの予備の差し替えで行う。状態機械・台帳・pool・worker・世代・時計・fadeはRust、C++は生成・破棄・state適用・graphへの着脱の薄いwrapに限る。最後に現行evolveと同じ条件で比べる）。物理instanceを1pluginだけ載せた未公開graphとすること、Host APIを完了callback型にすること、予備の数、UAPMDを呼ぶthread、evolveとの排他はAIの提案をuserが承認
- 関連: [ADR 0017](0017-evolving-loop-prototypes.md)、[ADR 0020](0020-headless-clap-host-for-evolve2.md)

## 決定と理由

[ADR 0017](0017-evolving-loop-prototypes.md)の現行evolveは、UI/control threadから同期にpluginを生成し、切り替えのたびに旧instanceを破棄してから新しいinstanceを作る。予備poolはA/Bの2個という決定と両立しないため採らなかった。userはこれを置き換えず、clap-mml-render-tuiのgrid sequencerとclap-mml-play-serverが持つ構造（preloadの世代、論理slotから物理instanceへの台帳、種別ごとの予備の袋）を同じ形で持つevolve2を別に作り、構造の差だけを比べることを選んだ。evolve2はこの比較のために、ADR 0017が却下した予備poolを採る。

- **物理instanceは「1pluginだけを載せた未公開graph」とする。** slotへの着脱はevolve2の`Source`が返すgraphを差し替えるだけで、予備はgraphを持つが返さないのでDSPが呼ばれない。同時に処理されるのは現在音と次の音色の最大2個で、ADR 0017の上限を保つ。
- **予備は、要求されたことのある種別ごとに1個とし、追い出さない。** 種別ごとの物理数は「slot数 + 予備の目標数」を超えない。CLAPは同種のinstanceが0個になるとmoduleを外すので、予備はmoduleの再ロードも避ける。1個は仮の値である。
- **UAPMDの生成・state読込・破棄は、全部mainのtaskとして積む。** RustのworkerからUAPMDを直接呼ばない。CLAPはUI-thread要件が全非audio操作なので、別threadから呼んでも生成・state読込・破棄はmainへ回され、mainの停止時間は変わらなかった。直接呼ぶとlockの無い`RemidyAudioPluginHost::instances`やprocess全体の`current_path`にmain以外から触る。Host APIは要求を積んで即座に返り、完了をRustのcallbackで受ける。
- **evolve2を閉じるときは、Rustの`destroy`を別threadで呼び、mainは積まれたtaskを処理しながら待つ。** `destroy`は生成中の要求と捨てた読込の完了を待つが、その完了はmainのtaskから来るので、mainで同期に呼ぶと戻らない。
- **evolveとevolve2は同時に開けない。** 未公開graphを演奏する補修の`active`が1個だからで、どちらかが開いていれば他方の`open`はerrorにする。専用trackの作成と片付けは`evolution-mode.js`を共用し、通常の演奏snippetや`try-end-evolve`でどちらも終わる。
- **JSは決着を待つloopを持たない。** UAPMDの処理はmainのtaskで走り、GUIでは毎frameの`poll`がそれを処理するので、JSが待つとUIを占有し非同期の意味が消える。このため再生開始から最初の音色が準備できるまでは無音になる。

## 結果と制限

同じ固定stateの比較で、同じpluginへの切り替えは現行の生成約43msに対し、stateの再適用で約0.8msになった。別pluginでも予備の差し替えでSurge XT・TyrellN6は約1ms、Vaporizer2は約12ms（現行は約180msでUIを止める）になった。

固定stateの比較でUIを最も長く止めていたのは、生成でなくSix Sinesのstate読込だった。止めていたのはplugin自体でなく、UAPMDのCLAP hostがpluginからのrescanのたびに全parameterを読み直す処理で、evolve2のinstanceにだけそれをしないhostを付けて、固定stateでのSix Sinesの切り替えのUI停止は約1.6秒（新しいinstanceでは約4.9秒）から最大約51msになった（[ADR 0020](0020-headless-clap-host-for-evolve2.md)）。

固定stateの比較はcatalogを通らない。catalogから音色を選ぶ`try-evolve2`の流れでは、切り替えのたびに`repl.patchCatalog`がcatalogを読み直して全patchを分類し、UIを約1.5秒止めていた。分類済みの一覧をprocess内に1回だけ作って持つようにした後の`try-evolve2`の35秒の計測では、切り替えのUI停止はplugin自身（Surge XT・Six Sines）の約130〜190msだった。

evolve2には現行evolveが持つ1coreの稼働維持とTIME_CRITICALが無い。別pluginの比較でevolve2にだけworkerの破棄が残ったが、この差と重いmain taskの重なりのどちらが原因かは切り分けていない。
