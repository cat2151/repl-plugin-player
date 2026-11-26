# ADR 0001: patch の再現と試聴は plugin の state で行い、試聴は元の instrument を bypass した横に別 instance を足す

- 状態: 採用
- 決定者: user（patch browser を薄い API で QuickJS に出し、試聴から元に戻すまでを lib に書くこと・差し替え先 track を cell で指定すること・閉じた通知を player に `KitClosed` と同じ形で足すこと）。state で再現する方式・試聴の instance の置き方・cell を書く時機は AI の提案
- 関連: [0002](0002-cells-address-instruments-by-track.md)（`use` の track と楽器の置き換え）、[cat-smf-editor ADR 0006](../../../cat-smf-editor/docs/adr/0006-instrument-layers-and-patch-state-cache.md)（同じ `--restore-state` で patch を再現する）

## 決定

- cell には plugin 名と player の catalog 上の表示パスと track だけを書く（`use({format, plugin, patch, track})`）。`use` はそれを `repl.patchState` で plugin の state にし、その track の楽器に読ませる（別 plugin なら楽器を置き換える。ADR 0002）。`repl.patchState` は cat-plugin-player の `--restore-state` に `PatchReference` を渡して state を得る。表示パスから実ファイルへの対応は `--patch-roots` の roots だけで組み、player の catalog file は読まない。
- browse 中の試聴は、player が `Selected` で渡す共有メモリの state をそのまま読ませる（`--restore-state` を呼ばない）。
- 試聴は、track の楽器（`instrumentOn(track)`、ADR 0002）を bypass し、同じ track に試聴用の instance を1つ足して state を読ませる。接続を編集した track では足した instance がどこにもつながらないので、graphInput の event → 試聴 → graphOutput に直接つなぐ。別 plugin の patch に移るときは試聴用の instance だけを作り直す。browser を閉じたら試聴用の instance を消し、元の instrument の bypass を browse 前の値に戻す。元の instrument には state を読ませない。
- browse の結果の cell は、browse の script が返った後に1つだけ挿入する（エラーなく試聴できた最後の patch）。cell には browse した track を書く。track は browse が `repl.patchBrowser.open({track})` で渡し、Rust が開いている間だけ持つ。挿入した cell は実行しない。
- state を instance に読ませるのは uapmd script API の `instance.loadState`。上流のままでは返らないので、uapmd の補修 [`script-api`](../../patches/uapmd/script-api/README.md) で完了まで main thread の queue を回して待つ形にする。

## 理由

- uapmd の `getPresets()` で名前を引く方式では、Dexed・VAST2・Six Sines・Floe は0件、Surge XT でも起動ごとに名前が揺れ、catalog の大半を再現できない。`--restore-state` は player の catalog の全 plugin で state を返し、1件 0.1 秒前後で済む。
- `--patch-roots` だけで組んだ対応は、player の catalog 全件（約3.6万件）で catalog の解決結果と一致した。catalog file の形式に依存しなくて済む。
- 共有メモリの state は player が選択時に作り終えているので、試聴のたびに別 process を起こすより速い。
- 元の instrument に触らないので、閉じた後の音色は browse 前と parameter 単位で一致する。browse 前の state を保存して戻す必要も無い。
- browse の script は GUI の thread で回り、その間は文書を描けない。選ぶたびに cell を置き換えた最終形と、終わってから1つ書いた結果は同じになる。
- uapmd script API の `instance.loadState` は上流のままでは main thread で完了を待つだけで、完了通知も同じ thread の queue に積まれるため返らない。待つ間に queue を回せば返り、上流の `project.load` と同じ待ち方なので、上流へ報告できる直し方になる。

## 却下した案

- **state を cache する。** 再現は1件 0.1 秒前後で足りる。初回だけ遅い plugin（sforzando で約10秒）があるが、cache が無くても成立する。
- **player の catalog file を読んで表示パスを解決する。** player の cache 形式に repl が依存する。
- **元の instrument に試聴の state を読ませ、閉じたら browse 前の state に戻す。** browse 前の state を取る手段（`saveState`）が止まり、戻した結果が元と一致することも保証しにくい。
- **アプリの `uapmd_host.dll` に読込用の関数を足す。** 補修無しで済むが、uapmd に既にある `loadState` と同じ役目の API を二重に持ち、上流が直っても残る。

## 結果

- 試聴の instance は track の effect より後ろに付くか、graphOutput に直接つながるので、試聴の音には track の effect が掛からない。
- browse 中は REPL の窓が描画も入力も止まる。閉じるまで窓も閉じられない。
- 試聴・再現の state file は process ごとに1つずつ `%TEMP%\repl-plugin-player\` に上書きで残る。
