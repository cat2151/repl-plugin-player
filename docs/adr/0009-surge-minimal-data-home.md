# ADR 0009: Surge XT は最小の data home で起動し、音色は patch のパスで指す

- 状態: 採用
- 決定者: user（兄弟 repo と同じ `SURGE_DATA_HOME` の最小化を入れること・Surge の preset 一覧を失う代わりに snippet を patch のパス指定へ寄せる案を選んだこと・名前で探す `getPresets()` をやめること・関数名を `prepareInstrument` にすること）。`prepareInstrument` を options 形式にして楽器の音色指定を1本にすること・`prepareEffect` と分けたままにすることは AI の提案
- 関連: [0001](0001-patch-browse-by-plugin-state.md)（patch を state で再現する）、[cat-smf-editor ADR 0012](../../../cat-smf-editor/docs/adr/0012-surge-minimal-data-home.md)（同じ最小構成）

## 決定

- process の最初に、`cat-plugin-player/shared/surge_data_home.rs` で `SURGE_DATA_HOME` を最小構成へ向ける（`patches_factory` は `Templates` だけ、`patches_3rdparty`・`fx_presets`・`wavetables` は空）。cat-plugin-player を子 process で起こすとき（patch browser・`--patch-roots`・`--restore-state`）は元の値へ戻す。
- snippet の音色は `prepareInstrument({format, plugin, patch, track})` の `patch` に catalog の表示パスで書く（`patches_factory/Templates/Init Saw.fxp`）。uapmd の `getPresets()` / `loadPreset()` で名前を引かない。

## 理由

- Surge XT は instance を作るたびに約3000件の patch を走査する。最小構成ではこれが無くなり、`try-init-saw` を process 起動から流し終えるまでが約4.6秒から約3.9秒になった（HDD が温まった状態）。
- 最小構成では Surge の preset 一覧が0件になり、`getPresets()` で名前を引く方式は成り立たない。一覧と走査は同じものなので、片方だけ残すことはできない。
- 名前で引く方式は、同じ名前が複数あると候補の順番に依存する（`Attacky` は factory と 3rd-party に1つずつある）。表示パスなら1つに決まる。

## 却下した案

- **起動時だけ最小構成にし、画面を開くときや一覧が要るときは通常の data home で作り直す（cat-plugin-player の方式）。** Surge の DLL を外してから作り直す必要があり、複雑になりすぎる。
- **patch は残し、`fx_presets`・`wavetables` だけ空にする。** 主な原因である patch の走査が残る。
- **`prepareInstrument` と `prepareEffect` を1つの関数にする。** uapmd ではどちらも track の graph に作る instance で同じだが、つなぐ位置・置き換えるか再利用するか・音色の初期化が3つとも違い、flag で分岐するだけになる。

## 結果

- `showUI()` で開いた Surge の画面では、patch・wavetable・FX preset の一覧が空になる。
- 表示パスの解決に、process ごとの初回だけ cat-plugin-player の `--patch-roots`（約0.9秒）が掛かる。
