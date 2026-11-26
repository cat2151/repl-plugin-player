# ADR 0021: 音色・effect の state は repl の process 内の Rust で作り、UAPMD・`uapmd_host.dll` には汎用の口だけを足す

- 状態: 採用
- 決定者: user（plugin ごとの処理は Rust に置き、UAPMD の補修と `uapmd-host/uapmd_host.cpp` には plugin 名を知らない汎用の口だけを入れること。手本を clap-mml-play-server の grid-sequencer とし、鳴っている instance に直接音色を入れること。clap-mml-play-server を server process として使わず、cmrt の crate を library として呼ぶこと。対象を名前を書いた cell と `patchJob` に限り、browser の経路は変えないこと。名前を書いた cell と evolve2・evolution の音色準備は、sforzando を含めて child process に依存しないこと。sforzando の template を plugin ごとに1つ持つこと。Floe・Dexed の読込を待たずに返すこと。筋の基準を「simple で ETC 原則などを遵守し、child process に依存しない」とし、そのための修正コストを払うこと）。音色を instance へ入れる Rust の関数を1つにして track と evolve の host が明示的に呼ぶこと（user 承認）、`--patch-roots` を起動時に別 thread で1回取ること、Surge XT Effects に事前の表を使うことは AI の提案
- 関連: [ADR 0019](0019-evolve2-async-pool.md)、[ADR 0020](0020-headless-clap-host-for-evolve2.md)、補修の説明書（[clap-plugin-handle](../../patches/uapmd/clap-plugin-handle/README.md)・[clap-sysex-input](../../patches/uapmd/clap-sysex-input/README.md)）

## 決定と理由

名前を書いた cell（`patch.use`・`effect.use`）と `patchJob` は、cat-plugin-player の `--restore-state` を child process で起動して state file を得ていた。1回あたり child の起動〜終了だけで約 95ms、sforzando・Floe では一時 instance の生成で約 700ms かかり、その間 UI thread が待っていた。これを repl の process 内で作る。

- **plugin ごとの処理は Rust に置き、cmrt の crate（`cmrt-patches`・`cmrt-core`・`cmrt-server-config`）を呼ぶ。** 変換だけで済む音色（Surge XT・Six Sines・Tyrell N6・Zebralette3・Vaporizer2）は file を変換して書く。sforzando と effect（Dragonfly・Shu・Voyage Voyage・TONE3000）は、生成直後の instance の state（template）に音色を合成する。Surge XT Effects は cat-plugin-player が保存した「preset の SHA-256 → state」の表を引き、鳴っている instance で range を測らない。
- **音色を instance へ入れる Rust の入口は1つ（`src/patch_state/into.rs`）で、track も evolve の host もそれを明示的に呼ぶ。** `prepare`（どの thread でもよい）が音色を確かめ、instance の要らない state を作っておく。`into`（CLAP の main thread）が instance を受け取り、Floe・Dexed は直接入れ、それ以外は読ませる state を返す。`loadState` は呼び出し側（track は JS、evolve は C++）に残し、UAPMD の `loadState` を通す。host は受け取った bytes を常に `into` に渡し、plugin の state か否かを見て振り分けない。
- **UAPMD・`uapmd_host.dll` に足すのは、plugin を名指さない口だけ。** instance の生の `clap_plugin*`（Floe の独自 extension と clap の `state` を Rust から呼ぶため）、instance への MIDI SysEx の入力（Dexed の voice を送るため）、evolve の host が instance を作った後・鳴らす前に Rust の `into` を呼ぶ関数 pointer（`repl_set_patch_into`）、track に属さない新しい instance を1回だけ作って Rust に渡す口（`repl_fresh_plugin`）。どれも「どの plugin に何をするか」は Rust が決める。
- **evolve へは `patchJob` が `prepare` の結果（音色の参照と、作ってあれば state）を file に書いて渡す。** 「instance ができた後、鳴らし始める前」は C++ の読込 task の中にしか無く、JS から READY を見て入れると再生が先に取る競争が残る。file の受け渡しを変えないので evolve の ABI は変わらない。この file は plugin の state ではないので、track の instance へ `loadState` しない。
- **Floe・Dexed は鳴っている instance に直接入れ、state file を作らない。** Floe は main thread の callback で読み進めるので、UI thread（UAPMD の main thread）で読み終わりを待たず、要求して返る。Dexed の SysEx は次の `process()` で渡るので、`activate()` 前には送られない。
- **template は plugin ごとに process で1つ、新しく作った instance から clap の `state` extension で取る。** 音色を読んだ後の instance の state を template にすると、前の音色の parameter が以後の組み立てに残りうる。新しく作った instance に入れるときはその instance 自身から取り、template が無いまま使い回しの instance に入れるときだけ `repl_fresh_plugin` で新しい instance を1つ作って取り、すぐ捨てる。

## 却下した案

- **cat-plugin-player と同じく child process で state を作る。** 1回ごとに child の起動と一時 instance の約 700ms が残る。
- **evolve の host に渡す file に固定の印を付け、host が state を読む直前に Rust が印を見て引き取る（state taker）。** 印で振り分ける約束が host と Rust に分かれ、その file を track の instance へ `loadState` すると壊れた state が渡る。track 側は Rust が instance を知らないので、JS が検査と state 取得の2段で呼ぶ必要が残った。
- **evolve の Host API に「生成後の hook」を足す。** 当時あった現行 evolve（`evolution.*`）は `evolve-dll` を通らなかったので関数 pointer の口は結局要り、その上に ABI の変更と worker thread から main thread への往復が重なる。
- **Rust（cmrt の clack host）で `.clap` を直接読み、template 用の一時 instance を作る。** repl の process に plugin host が2つになり、crash 時の切り分けが難しくなる。
- **plugin 固有の loader を UAPMD・`uapmd_host.dll` の C++ に移す。** user は plugin ごとの処理を Rust に置くことを選んだ。C++ に置くと plugin を足すたびに native の build と補修が要る。
- **ARA を有効にして既存の `nativeHandle` で生の pointer を得る。** ARA の SDK を取りに行く build 設定の変更になる。
- **sforzando の組み立てを repl に写す。** cmrt と解釈系が二重になる。cmrt 側の関数を公開して呼ぶ。

## 結果と制限

1回あたりの所要は、変換だけの音色の `patchState` が約 90ms 減り（Surge XT 約 95ms → 9ms）、sforzando・Floe の既存 instance での音色切替が約 800ms から数十 ms 以下になった。template の無い使い回しの sforzando への最初の切替は、新しい instance を1つ作る分（約 160ms）かかる。起動直後の `--patch-roots` は child process のまま残る。

Floe・TONE3000 は読込の完了を待たないので、直後に鳴らした note は前の音色か無音になりうる。Surge XT Effects の表は cat-plugin-player が一度も保存していなければ無く、表に無い preset とともに child process に回る。非 CLAP の plugin など player だけが作れる state も child process に回る。
