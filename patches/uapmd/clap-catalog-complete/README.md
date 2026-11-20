# cache に無い CLAP bundle を scan 後に補う

## 原因

UAPMD の `PluginScanTool` は、plugin list cache に CLAP の slow entry が1つでもあると、
`forceRescan` でない限り CLAP の bundle を列挙しない（`PluginScanTool.cpp` の
`if (!forceRescan && hasCachedSlowEntries) continue;`）。GUI の refresh は fast scan だけを走らせ、
plugin 不足時の `scan` は `forceRescan` を立てない。このため、cache を作った後に入れた CLAP plugin は、
cache を消すまで一覧に出ない。

## 処理と適用

`repl_clap_catalog::complete(host)`（`clap_catalog.hpp`）は、cat-plugin-player の
`completeClapCatalog`（`shim/plugin_catalog.cpp`）と同じ処理をする。
host の catalog に bundle path が無い CLAP bundle だけを、blocklist を除いて `scanBundle` で読み、
増えたら CLAP・VST3 の entry を plugin list cache に保存し直す。
cache から消えた bundle（uninstall 済み）は取り除かない。

bridge の `scanningCompleted` callback（scan worker 上）が scan の成功後に呼び、cache が変わったら
`reloadPluginCatalogFromCache()` で host の catalog を作り直してから、scan 完了を UI thread へ伝える。
起動時に cache から読むだけで scan しない経路（CLI・e2e、GUI の refresh 前）では呼ばれない。

`apply.cmake` は依存checkoutを変更せず、`clap_catalog.cpp` を `uapmd-plugin-hosting` に足す。
通常の `python scripts/build.py` で再clone・既存キャッシュのどちらにも適用する。

## 適用条件と失敗時

`apply.cmake` は上流 `PluginScanTool.cpp` に上記の skip がちょうど1つあることを確かめ、
無ければ補修名、対象ファイル、見つかった数、上流revision、本書の場所を表示して停止する。
停止したら、上流が cache にある format の bundle 列挙を省く条件を変えたか読み、
新しい bundle が full scan で載るなら撤去、載らないなら anchor を新しい条件に合わせる。

検証した上流revision: `4ec5c3b1c30130fdd3b7d4b375082273247e561f`。

## 検証

`python patches/uapmd/clap-catalog-complete/regression.py`（`python scripts/build.py` 後、アプリを閉じて実行）。
実データの cache から Surge XT 以外の CLAP bundle を1つ外して実 exe を起動し、
UAPMD の JS 入口 `__remidy_scan_tool_perform_scanning` で full scan を走らせ、
その bundle が host の catalog と cache に戻ることを確かめる。cache は終了時に元へ戻す。
上記revisionの既存Windowsビルド領域で、補修前は full scan 後も bundle が戻らず、補修後は戻ることを確認した。
新規環境での全依存取得は検証範囲に含めない。

## 撤去

上流が cache 済みの format でも新しい bundle を列挙するようになったら、
この補修を外した状態で `regression.py` が通ることを確かめて撤去する。

作者への報告状態: 未報告。提案文は `upstream-report.md`。外部送信は行わない。
