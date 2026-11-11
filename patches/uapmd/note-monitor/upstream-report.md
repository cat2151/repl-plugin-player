# 提案: プラグイン入力ノート状態の観測API

UAPMD revision `a4a96938eb31ddcb70209d62caf0dcc6ea9d3abe` を利用する
REPL Plugin Playerで、実際にプラグインへ渡ったNote On/Offをミニ鍵盤に表示しています。
保存済みTimelineイベントやsendNoteOn呼び出しの履歴では、エンジン停止中のキューや
Stop flushによる解除を含めた入力状態を判断できません。

現在はアプリ内の生成コピーで、AudioPluginNodeImpl::processAudioの
trackInputEvents直後へ読み取り専用の観測を追加しています。依存checkoutは変更していません。
group/channel/pitch別の固定長状態をaudio threadで更新し、lock-free snapshotでGUIへ渡します。
linear/full DAG両方と直接送信・タイムライン再生を同じ位置で観測できます。

オシロスコープのため、同じ生成コピーでprocessAudio成功直後の第1busのL/Rと
入力UMPをアプリの有界queueへ渡すcallbackも追加しています。MIDI音高に合った
連続窓の表示が目的であり、256sampleのmono出力履歴だけでは代替できません。
callback登録は処理開始前、解除・破棄は処理停止後とし、RT側の確保やロックは追加しません。

将来、入力ノート状態の安全なsnapshot取得か、RT制約が明確な入力観測APIがあれば、
このローカル拡張を撤去できます。入力gate状態と、同じ入力に対応する連続出力波形を
安全に観測できるAPIを希望します。プラグインの発音中voice数を求めるものではありません。

回帰検証: エンジン停止中の送信、和音、例外後の入力、Stop/restart、MIDI 1/2、
チャネル別解除、velocity-zero、All Notes Off / All Sound Offを実機Surgeで確認しています。
