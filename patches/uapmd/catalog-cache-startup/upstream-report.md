# 提案: plugin list cache だけで host の catalog を作る公開 API

`AppModel` の起動時 scan は fast scan を走らせ、完了後に `reloadPluginCatalogFromCache()` で
host の catalog を作り直します。この reload も内部で fast scan を走らせるため、
cache があっても起動時に CLAP・VST3 の探索パスの走査が2回起きます。
Windows の手元環境（CLAP 17件・VST3 7件）で1回 約0.7秒、cache の読込だけなら 約2ms でした。

host の catalog は `RemidyAudioPluginHost` の private member にあり、cache を読むだけの公開 API がありません。
アプリ側では host の source の生成コピーに、cache を `AudioPluginCatalog::load` で読むだけのメソッドを足し、
起動時はそれで catalog を作って、走査は UI が空いたときに後回しにしています。

`AudioPluginHostingAPI` に「cache だけで catalog を作る」入口があると、このソース差し替えを撤去できます。
また `reloadPluginCatalogFromCache()` は cache ファイルの有無で分岐していますが、両方の分岐が同じ処理です。

確認revision: `4ec5c3b1c30130fdd3b7d4b375082273247e561f`。未報告。
