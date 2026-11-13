# ADR 0002: cell は楽器を track 番号で指し、`use` は track の楽器を置き換える。起動時の楽器も init script で用意する

- 状態: 採用
- 決定者: user（差し替え先の track を cell の中で指定すること・track を省いた `use` はエラーにすること・起動時の楽器を native でなく init script と lib で用意し `globalThis.surgeId` を消すこと・track 番号は役割の分かる説明変数を経由すること・effect 付き track の置き換えを「graph のつなぎ直し → effect の作り直し → 補修で並べ替え API」の順に試すこと）。track 番号で話す形・「今の track」の global を作らないこと・cell をブロックで包むこと・別 plugin なら置き換えることは AI の提案を user が承認。楽器の決め方・scope の付け替え・起動失敗の分岐・uapmd の trackId を lib が track 番号から引くことは AI の設計
- 関連: [0001](0001-patch-browse-by-plugin-state.md)、[cat-smf-editor ADR 0002](../../../cat-smf-editor/docs/adr/0002-track-patches.md)（track ごとに音色を持つ）

## 決定

- cell 同士は track 番号で話す。楽器の instance id を global に保存して cell 間で渡さない。「今着目している track」を表す共通の global も作らない。
- snippet の cell は `{ ... }` で包み、track 番号は `const loopTrack = 0;` のような説明変数を経由して lib に渡す。
- uapmd の JS API は track・clip を文字列の `trackId`・`clipId` で受ける。track 番号から `trackId` への変換は `lib/tracks.js` の1関数（`getTrackInfos()` の並びの位置で引き、無ければ `patch_browse_00` のエラー）だけが持ち、lib は uapmd を呼ぶ直前に変換する。cell と lib の引数は track 番号のまま。repl 独自の API（`repl.track.*`・`repl.timeline.publishLoopClips`・timeline view の読取）も track 番号のまま。
- track の楽器は `lib/plugins.js` の `instrumentOn(track)` で引く。`ensureDagGraph(track)` の後の track graph の接続で、graphInput からの event 接続の先の plugin、無ければ audio 接続の先の plugin。
- `lib/patch.js` の `use({format, plugin, patch, track})` は track を必須にする。track の楽器が同じ plugin ならそれに state を読ませる。別の plugin なら同じ track に新しい instance を作り、古い楽器が持っていた接続を新しい楽器へ付け替えて張り直し（graph のつなぎ直し）、古い楽器を dispose する。
- 起動時の楽器は init script `snippets/builtin/init.js` が `ensureInstrument(startTrack, {format, plugin})`（track に楽器が無いときだけ作る）で用意する。Rust は scan の後にこの cell を1回評価する。plugin 名は init script に書き、lib には書かない。
- Rust は init script の失敗のうち、`catalogEntry` が `missingPlugin` を付けたエラーだけを「plugin が catalog に無い」として full scan・install 画面へ回す。他の失敗はそのままエラーにする。
- scope（オシロスコープ）の対象は script から `repl.scope.setTarget` で付け替える。`ensureInstrument` は対象が無いとき作った楽器を、`use` は古い楽器が対象なら新しい楽器を対象にする。

## 理由

- user が見て書けるのは track 番号で、instance id は楽器を置き換えると変わる。cell 同士が global の instance id でつながっていたため、別 plugin の音色を選ぶ cell が新しい空の track に楽器を作り、loop の track の音色が変わらなかった。
- 共通の「今の track」の global は同じ型の問題を作り直す（cell の結果が実行の順番と履歴で変わる）。GUI には選択中の track が無く、その値は画面に見えない。1つの cell で2つの track を使う snippet もある。
- cell は1つずつ別の global script として評価される。トップレベルの `const`/`let` は同じ cell の再実行で `redeclaration` エラーになり、`var` は global に漏れて cell 同士を暗黙につなぐ。ブロックで包めば `const` のまま再実行でき、最後の式の値も結果に出る。
- `getTrackInfos()` の `nodes` は nodeId の文字列順で信号の順ではなく、catalog に instrument か effect かの区別も無い。JS から信号の順を読めるのは track graph の接続だけ。
- graph のつなぎ直しは effect を作り直さないので、effect の残響が切れず、effect の state を保存する必要も無い。実測で、つなぎ直した後の新しい楽器に effect が掛かった。
- 起動時の楽器も「track N の楽器」として同じ lib で扱え、起動直後に音が出る UX を保てる。
- どれが楽器かは native では決まらない。scope の対象を script から付け替えれば、楽器の決め方を JS と native の2か所に持たずに済む。
- `missingPlugin` の目印で分岐すると、Rust は plugin 名もエラー文言も知らずに済み、init script の書き間違いで full scan や install 画面へ誘導しない。

## 却下した案

- **track を省いた `use` は、同じ plugin の instance を探すか新しい track に作る。** 黙って新しい track を作ることが今回の不具合そのもので、エラーにすれば書き忘れにその場で気づける。
- **新しい楽器を `create(track)` して古い楽器を dispose するだけ。** uapmd は作った plugin を chain の末尾（effect の後ろ）に足すので、effect が新しい楽器に掛からない。
- **track の effect を同じ plugin・同じ state で作り直して新しい楽器の後ろに並べる。** uapmd の補修なしで確実に並ぶが、置き換えの瞬間に effect の残響が切れる。effect の state を保存する `saveState` も上流のままでは返らない。
- **track 内の node を並べ替える API を uapmd の補修で足す。** graph のつなぎ直しが成り立ったので補修は要らない。
- **cell が `trackId` を持ち、lib へ渡す。** `trackId` は user が画面で見て書ける番号ではなく、cell が番号で話す決定と食い違う。cell 同士が文字列 id でつながる形にも戻る。
- **uapmd を補修して track 番号で受ける JS API を残す。** 上流の API 全体に掛かる補修になり、上流が変わるたびに追従が要る。変換は script の1関数で済む。
- **native が起動時に楽器を作り、id を global に出す。** cell が global の instance id でつながる形そのもので、楽器を置き換えると id が古くなる。
- **native が instance の作成・削除の通知で scope の対象を追う。** どれが楽器かは native では決まらない。

## 結果

- `instrumentOn` を通った track は DAG の graph になる（変換しただけでは音も `create` の挙動も変わらない）。`use` で別 plugin に置き換えた track は接続を編集した custom topology になる。custom topology の track に lib を通さず `create(track)` した plugin はどこにもつながらない。lib の `prepareEffect` と browse の試聴は、作った plugin がつながっていなければ自分でつなぐ。
- 置き換えの間、uapmd は plugin の作成・削除のために transport を止め、loop の音が一瞬（約150ms）途切れる。
- 古い楽器の出力のうち bus 0 以外は引き継がない。新しい楽器にその bus があるとは限らないため。
- effect だけの track では、先頭の effect を楽器として扱う。
- init script の plugin が catalog に無いときの install 画面と CLI のエラーは Surge XT 用の文言のまま。
- E2E は init script の内容（plugin・音色）を前提にせず、テスト専用の init cell で起動する。本物の init script は「track 0 に楽器が1つでき、音符が届き、scope の対象になる」だけを見る。
