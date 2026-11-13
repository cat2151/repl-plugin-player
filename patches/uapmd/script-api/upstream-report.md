# 提案: UapmdJSRuntimeへの同期host関数登録

アプリから既存Rust crateのMML/コード進行変換を呼ぶ際に、JS runtimeへhost関数を
登録する公開入口がありませんでした。アプリ側ではruntimeソースの生成コピーに
JSON文字列を同期往復するcallbackを1つ登録して対応しています。

上流にhost関数を登録できる公開APIがあると、このソース差し替えを撤去できます。
音声処理やSMFインポートの仕様変更は不要です。callbackはcontrol thread上で実行し、
引数・戻り値の所有権と例外境界を明示できる形を希望します。

`instance.loadState(filepath)` も返りません。`AppModel::loadPluginStateSync` の User origin の
経路が `loadPluginState` の完了を `future.get()` で待つだけで、完了通知（`setPluginState` の前の
`requestState` の callback を含む）が同じ main thread の queue に積まれるためです。Internal origin の
経路や `__remidy_project_load` と同じく、待つ間 `remidy::EventLoop::processQueuedTasks()` を回すと
返ります。アプリ側では生成コピーの `__remidy_instance_load_state` をその形に置き換えています。
`savePluginStateSync` も同じ待ち方です。

確認revision: `4ec5c3b1c30130fdd3b7d4b375082273247e561f`。未報告。
