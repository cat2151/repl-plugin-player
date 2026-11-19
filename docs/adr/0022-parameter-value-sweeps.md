# ADR 0022: 音色の読込のたびに全 parameter の値を読み直す処理を止め、値は読む側が毎回 plugin に聞く

- 状態: 採用。回帰テストの閾値 150ms は暫定値
- 決定者: user（track の Six Sines で音色 cell を実行しても UI を止めないこと。音色を変えた後も、track の instance の parameter の値が UI と script から新しい音色の値として読めること）。止める処理の選択（rescan の VALUES と timeline の値の cache の2か所、instance 生成時の一覧作りは残す）、補修名 `parameter-value-sweeps`、閾値は AI の設計
- 関連: [ADR 0020](0020-headless-clap-host-for-evolve2.md)、[補修の説明書](../../patches/uapmd/parameter-value-sweeps/README.md)

## 決定と理由

track の instance へ音色（state）を入れると、UAPMD は main thread で全 parameter の値を4周読んでいた。plugin からの `paramsRescan(CLAP_PARAM_RESCAN_VALUES)` ごとに1周（Six Sines は `stateLoad` の中と `onMainThread` で2回 rescan する）と、state を変えた後の `TimelineFacadeImpl::refreshPluginParameterCache` の2周。Six Sines は parameter が 2554 個で、`get_value` 1回が plugin の中で約 0.29ms かかるので、同じ instance への音色 cell 1回で UI が約3秒止まった。

- **値を読む側は全部、その場で plugin に聞く。だから読み直しの2か所を止める。** script UI の slider と script の `getParameterValue` は `getParameter` → `paramsGetValue` で毎回 plugin に聞き、`getParameters()` は値を含まない。rescan の読み直しが配る通知の受け手（JS の parameter 更新の列、仮想 MIDI device への NRPN）は、この app では取り出されないか作られない。timeline の `plugin_parameter_values_` には読む所が無い。
- **rescan は VALUES だけ止め、ALL・INFO・TEXT では一覧を作り直す。** 一覧（名前・範囲・段階値の名前）は値と違い、読む側が plugin に聞き直さない。
- **timeline の cache は、何もしない形でなく、その instance の分を消す形にする。** 後で読む所が足されたとき、古い値ではなく「無い」が見える。
- **instance 生成時の一覧作り（`pluginInstanceAdded` の `parameterMetadataList()`）は残す。** CLAP の `getParameter`・`setParameter`・`enqueueParameterRT` は一覧を作らずに引くので、一覧が無いと値の読み書きが黙って失敗する。`enqueueParameterRT` は audio thread から来るので、そこで遅れて作ることもできない（`params.get_info` は main thread 専用）。

## 却下した案

- **VALUES の rescan で「値が古い」印だけを付け、読むときにその parameter だけ取り直す。** 読む側が毎回 plugin に聞くので、印を読む者がいない。印を付けない形と同じになる。
- **一覧がまだ作られていない instance でだけ VALUES の rescan を何もしない。** track の instance の一覧は `pluginInstanceAdded` が先に作るので、ほぼ何も減らない。
- **clap-helpers の検査の水準を下げる。** `paramsGetValue` を検査つきで呼んでも `get_value` を直に呼んでも 2554 回の所要は同じで、費用は plugin 自身の中だった。
- **読み直しを別 thread へ移す。** CLAP の `params.get_value` は main thread 専用。
- **instance 生成時の値の1周（約 0.75 秒）も外す。** inline の header の関数なので、header を include する UAPMD の translation unit 7本を全部写さないと class の定義が食い違う。外しても新しい instance は一覧作りの約 2.5 秒が残る。
- **evolve2 と同じく、総なめしない host（ADR 0020）を track の instance にも付ける。** INFO・ALL の rescan で一覧が作り直されず、track の parameter の表示が古くなる。

## 結果と制限

track の Six Sines への音色 cell の UI の止まりは、同じ instance で約3秒から約 66〜128ms、新しい instance で約 6.3 秒から約 3.4 秒になった。同じ instance に残るのは Six Sines 自身の `stateSave`（undo 用と保存用の2回）と `stateLoad` の本体、新しい instance に残るのは一覧作り（`paramsValueToText` 8725 回を含む）と値の1周。音色を変えた後に読む値は新しい音色の値で、最初の音色へ戻すと最初の値に全部一致する。

state の読込で変わった値は、`parameterChangeEvent`・`parameterUpdateEvent` の通知として流れなくなる。通知で値の変化を受け取る機能（`uapmd.sequencer.getParameterUpdates`、仮想 MIDI device）を使い始めるときは、音色の読込の後に読む側から値を取り直す。回帰テストの閾値は、残りの約 66〜128ms が補修の対象外なので、補修前の約3秒と分けられる 150ms にしている。
