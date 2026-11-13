# 起動時のプラグイン一覧を plugin list cache から読む

## 目的と適用

UAPMD の `AppModel` は起動時に fast scan を走らせ、完了後に host 側の catalog を
`reloadPluginCatalogFromCache()` で作り直す。どちらも CLAP・VST3 の探索パスを毎回走査するので、
cache があっても Ready 前に fast scan が2回走る（実測で1回 約0.7秒、cache の読込は 約2ms）。
host の catalog（JS の `uapmd.catalog` と instance 生成が引く物）は `RemidyAudioPluginHost` の
private member にあり、cache だけを読む公開 API が無い。

この補修は `RemidyAudioPluginHost` に `replLoadCatalogFromCache()` を1つ足し、
`repl_catalog_cache::load(host)`（`catalog_cache.hpp`）から呼べるようにする。
cache が無い・plugin が0件・読めないときは false を返し、catalog は空のまま残る。

bridge は起動時にこれを呼び、true なら初回 scan を始めずに Ready まで進める。
GUI は Ready 後に入力が2秒途切れたら `refresh` で UAPMD の初回 scan
（`notifyUiReady` / `notifyPersistentStorageReady`）を1回だけ始める。
scan 中の JS 評価は完了まで待つ（scan が別 thread で host の catalog を作り直すため）。
CLI と e2e は refresh を始めない。false のときは従来どおり起動時に初回 scan を待つ。

調査した上流revision: `4ec5c3b1c30130fdd3b7d4b375082273247e561f`。
`apply.cmake` は依存checkoutを変更せず、`generate.py` で host の header と source のコピーを
`target/shim/catalog-cache-startup/` に生成し、`uapmd-plugin-hosting` のビルド対象を差し替える。
header を include するのはこの source だけなので、header のコピーは他の翻訳単位に影響しない。
通常の `python scripts/build.py` で再clone・既存キャッシュのどちらにも適用する。
元ソース変更時にはCMakeが再構成して生成し直す。

## cache だけでは載らない plugin

UAPMD の cache は、読込に DLL のロードが要る plugin だけを保存する。
`moduleinfo.json` を持つ VST3 のような fast scan で見つかる plugin は cache に無いので、
refresh が終わるまで一覧に出ない。CLI では refresh しないので出ない。

## 検証

`python patches/uapmd/catalog-cache-startup/regression.py` で、上流ソースからの生成が2回とも同じ形になること、
生成済みコピーと anchor を変えた上流を拒否することを確かめる。
`cargo test --release` の `catalog_refresh` で、idle 2秒で1回だけ始まり、入力で待ち直すことを確かめる。
実機では起動ログの `bootstrap` の `catalogFromCache`、GUI の `catalog_refresh_started` /
`catalog_refresh_finished`（`catalogCount`）を見る。上記revisionの既存Windowsビルド領域で、
cache ありの Ready が 約2.2秒から 約0.65秒になること、cache を退避した起動が従来どおり scan して
cache を作り直すこと、`python scripts/e2e.py` の全 case が通ることを確認した。新規環境での全依存取得は検証範囲に含めない。

## 更新・撤去

適用失敗時にはパッチ名、対象、失敗条件、上流revision、本書の場所を表示して停止する。
上流の `RemidyAudioPluginHost` の宣言と `reloadPluginCatalogFromCache` の定義を調査し、検査を外して通さない。
上流に「cache だけで host の catalog を作る」公開 API が入ったら、それで同じ検証が通ることを確かめて撤去する。

作者への報告状態: 未報告。提案文は `upstream-report.md`。外部送信は行わない。
