# ADR 0010: cat-plugin-player.exe は、使う瞬間に無ければ README の release zip から取る

- 状態: 採用
- 決定者: user（配布 zip に同梱しないこと・取得元を cat-plugin-player の README が指す release zip にすること・起動時に調べず、exe を呼ぶ瞬間に無ければ専用 overlay を出し、download ボタンで exe の隣へ置くこと）。Windows 同梱の `curl.exe`・`tar.exe` で取ることは AI の提案
- 関連: [0001](0001-patch-browse-by-plugin-state.md)（patch browser と `--restore-state`）

## 決定

- `patch_browser::player()` が exe を見つけられなかったとき、その呼出しはエラーで返し、GUI が次のフレームで overlay を出す。download ボタンを押すと、README（main）から最初の `releases/download/…zip` のリンクを読み、zip から `cat-plugin-player.exe` だけをこの exe の隣へ置く。呼出しのやり直しは user が cell を再実行して行う。
- 取得と展開は `%SystemRoot%\System32` の `curl.exe` と `tar.exe` で行う。

## 理由

- スクリプトは GUI スレッド上で同期的に評価されるので、呼出しの中で user の操作を待てない。
- README のリンクを読むので、cat-plugin-player 側が README を更新すれば、この app を作り直さなくても取得する版が変わる。
- `curl.exe`・`tar.exe` は Windows 10 以降に同梱されていて、HTTP と zip の crate を足さずに済む。

## 却下した案

- **HTTP と zip の crate（`ureq`・`zip` など）を足す。** 依存とビルド時間が増えるのに、この用途では同梱の tool で足りる。
- **cat-plugin-player.exe を配布 zip に同梱する。** user が選ばなかった。

## 結果

- README のリンク先の版（現在は v0.1.0）と、`shared/` のソースを main から読むこの app とで、常駐プロトコルの版がずれることがある。
- exe の隣に書けない場所（`Program Files` など）に置いた場合、download はエラーになる。
