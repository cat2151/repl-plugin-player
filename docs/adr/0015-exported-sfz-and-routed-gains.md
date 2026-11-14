# ADR 0015: 保存stateの復元と発音・graph処理を別々に保証する

- 状態: 採用
- 決定者: user（cat-smf-editorから保存したprojectのSFZ drumが無音になる症状の修正とeffects保存を優先し、group選別で失われる時刻情報の補修と本ADRへの統合を指示）。補修方法はAIの判断
- 関連: [editor ADR 0023](../../../cat-smf-editor/docs/adr/0023-uapmd-project-import.md)、[player ADR 0005](../../../cat-plugin-player/docs/adr/0005-clap-input-note-dialect.md)

## 決定と理由

editorからのarchiveはsforzandoの音色stateを持ち、replでも元のprogramへ復元できていた。
しかし実際のdrumを短いclipでオフライン生成するとピークは約`1e-20`で実質無音だった。
UAPMDの参照revisionはMIDI-only入力にもnative CLAP Noteを送っていたため、state保存を変えても発音は直らない。
cat-plugin-playerと同じ入力portのdialect判定を使い、plugin名による例外を設けず、対応する形式のnoteを1回だけ送る。

effectsの保存では、音源ごとのgainと合流後のeffectを同じgraphで復元する必要がある。
UAPMDのDAGはgeneric builtinを受け入れる一方、そのruntimeを作らず全builtinを最後の直列処理へ置いていた。
そのためgainからeffectへの接続が落ち、保存された接続どおりの音を再現できなかった。
generic builtinはDAG内で1回だけ処理し、暗黙のtrack faderだけを全体出力の後に残す。
また、レイヤーを選別するUMP groupがそのままpluginの入力port番号になると、group 1以降はport 0しか持たない音源へ届かない。
保存したtrack内のgroup指定をMIDI-CI機器側のgroupで置き換えると、ノートの選別自体も一致しない。
割当groupで選別した後はinstance内のgroup 0へ戻し、割当のない複数port入力には触れない。
レイヤーgainをvelocityやeffect後のfaderへ置き換える案は、音色や非線形effectへの入力を変えるため採らない。

group選別とportの補修だけでは、音符の時刻も保存どおりになるという前提が不足していた。
editorを介さず作った単一音源のclipをSix Sinesでオフライン生成すると、group 0では0.1秒のnoteが鳴る一方、group 1では開始と終了が同時になり無音になった。
1秒のnoteもgroup 1では約0.85秒へ短縮されたため、音源への到達と発音の有無だけでは同じ演奏を保証できない。
原因は、選別処理が音源宛てのgroupを持たないUtilityのJR Timestampまで除外し、処理block内の時刻を失わせることだった。
groupによる音源選別はUAPMD自身の機能であり、正しいclipでもこの欠落は起きるため、editorの保存内容を迂回させず再生側で共通の時刻情報を保持する。
宛先を持つnoteの選別とportへの変換は維持し、時刻情報は元の順序のまま通す。音符の長さや処理blockの大きさに頼る回避は採らない。

両補修は依存checkoutを変更せず、検証したsourceから既定のtarget内へ生成する。
既存のparameter metadataとnote監視の補修結果を引き継ぎ、共通headerを使う翻訳単位のclass layoutをそろえる。
上流更新時は前提の一致を検査して停止し、上流の修正を確認してから更新・撤去する。
project・設定の保存形式は変更しないため、既存データの移行は不要である。
