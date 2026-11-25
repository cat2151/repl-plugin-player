# アプリのRust APIをJSへ接続する

## 目的と適用

UAPMDの公開JS runtimeにはhost関数を登録する入口がない。
MMLからSMFを生成する既存Rust crateを同期呼び出しするため、
`UapmdJSRuntime::registerConsoleFunctions` にJSON文字列の往復を行う
`__repl_host_call` を1つ登録する。SMF取込・再生処理は変更しない。

調査した上流revision: `9f708d3300b4a6f8109eee0fadccf286db8b7a45`。
`apply.cmake` は依存checkoutを変更せず、runtimeソースのコピーを
`target/shim/script-api/UapmdJSRuntime.cpp` に生成してビルド対象を差し替える。
通常の `python scripts/build.py` で再clone・既存キャッシュのどちらにも適用する。
元ソース変更時にはCMakeが再構成して生成し直す。

同じ生成コピーで、`__remidy_instance_load_state`（`uapmd.instance(i).loadState(file)`）の
`loadPluginStateSync` 呼出しを、`loadPluginState` を投げて完了まで main thread の queue を回して
待つ形に置き換える。上流の `loadPluginStateSync` は User origin では `future.get()` で待つだけで、
完了通知が同じ thread の queue に積まれるため返らない（`__remidy_project_load` は同じ待ち方で
queue を回している）。戻り値の形（`{success, error, filepath, instanceId}`）と undo 記録は変えない。
置換対象の呼出しが1つだけ見つからなければ、適用失敗として停止する。
`saveState` の同じ待ち方は、この app が使わないので置き換えない。

## 境界と生成ファイル

callbackはDLLをロードしたRustアプリが設定し、control thread上で同期呼び出しする。
Rustの戻り値は次のcallbackまで有効な文字列で、C++側が直ちにコピーする。
RustのエラーとpanicはJSONエラーになり、JS wrapperが例外に変換する。
公開する関数名は既存crateの名前を維持する。

同じcallbackで `repl.text(key, values)` の翻訳読取と `repl.ui.show/close` の
GUI定義登録を扱う。翻訳は起動時に検査したja/enカタログから取得し、
動的な文言は名前付きのvaluesを一度だけ差し込み、外部診断やユーザー文字列の中を再置換しない。
UI登録はJSONデータだけを保持する。callback中に同じJS runtimeへ再入せず、
parameterの読取・書込は通常のegui frameから行う。CLIではUI登録を拒否する。
UI定義と言語の保存設定は追加せず、既存のsnippet保存経路も維持する。

`getGeneratedMidiPath()` はLocalAppDataの
`repl-plugin-player/generated-midi` を必要なら作成し、重複しないフルパスだけ返す。
ファイルの生成は `save_midi_file` が行い、生成済みSMFは削除しない。
既存の保存形式・起動時復元・ユーザーsnippetは変更しないため移行は不要。

## scriptからの再生

`startPlayback()` は上流の `LatencyCompensationManagerImpl::startPlayback` を通り、
再生位置0から必要な内部prerollを準備する。事前の `jumpPlayback(0)` は不要。
追加すると停止flushが要求され、直後の再生で時刻0のノートを落とすため、
生成snippetとplay snippetでは呼ばない。クリップ位置は0のままとし、
固定の無音やscriptのsleepは追加しない。

`builtin/lib/tutorial.js` の `init({instanceId,preset,bpm})` は停止とpreset読込を完了し、
`play(input)` はMMLまたはコード進行からクリップを置換して再生する。
別セルでplayだけを呼ぶ場合はpresetや音量を書き戻さず、現在の音色を使う。
音量を工場presetの数値へ強制復元する処理は置かない。

上流のエンジン停止とMutationJobは非同期であり、enabled=falseやloadPresetの返却だけでは
完了を判定できない。アプリ所有のbridgeで `repl.audio.isProcessing()` に既存dispatcherの
`isAudioPlaying()` を、`repl.processEvents()` に既存control threadのイベントキュー処理を薄く公開する。
後者の1msはpoll時のCPU占有を抑えるためであり、楽曲の先頭をずらす待機時間ではない。
停止の判定、jobの完了・失敗の判定、10秒の期限はlib内に置く。停止済みtransportへさらにStopを
要求すると未消化flushが次の先頭ノートを消し得るため、再生中だけ要求する。
この処理は依存checkoutや保存形式を変更しない。

## 検証

`cargo fmt --check`、`cargo clippy --release -- -D warnings`、
`cargo test --release`、`python scripts/build.py` を実行する。
`python scripts/e2e.py mml-clip chord-clip snippets partial-run timeline` で、
2つのsnippetをそれぞれReadyから単独実行し、ノート列、和音の時刻、SMF保持、
再生信号、パスの一意性、不正引数からの復帰、既存操作の回帰を検証する。
回帰のscriptはこのdirectoryの `mml-clip.js`・`chord-clip.js` にあり、生成パスが
LocalAppDataの `repl-plugin-player/generated-midi` 配下かは `scripts/e2e.py` が見る。
`python scripts/e2e.py state-load` は、準備済みinstanceのstateをfileに書き、parameterを1つ動かしてから
`loadState` で読み戻し、10秒以内に `success:true` で返ってparameterが元に戻ること、
読めないfileが `success:false` で返ることを確かめる（検査は `state_regression.rs`、流すJSは `state-load.js`）。
`python scripts/e2e.py demo-play tutorials` はtest専用の休符なしMML・コード進行で
先頭音のプラグイン入力とmaster出力を調べ、別音色へ変更後のplayでパラメーターが保たれること、
再初期化、再生中の置換、loopからの切替を確認する。ユーザーが編集するsnippet自体には
音符・音色の期待値を持たせず、try系は各2回の実行成功だけを確認する。
上記revisionの既存Windowsビルド領域で、2つのsnippetの先頭音を含む全音の
プラグイン入力とmaster出力信号を確認した。スピーカー出力の収録と、
新規環境での全依存取得は検証範囲に含めない。

## 更新・撤去

適用失敗時にはパッチ名、対象、失敗条件、上流revision、本書の場所を表示して停止する。
上流runtimeの関数登録箇所とtargetのソース一覧を調査し、検査を外して通さない。
上流にhost callback登録APIが追加されたら、それで同じ検証が通ることを確認して撤去する。
上流の `loadPluginStateSync` が待つ間に queue を回すようになったら、`loadState` の置き換えだけを外し、
`python scripts/e2e.py state-load patch-use patch-browse` が通ることを確かめる。

作者への報告状態: 未報告。提案文は `upstream-report.md`。外部送信は行わない。
