# ADR 0012: 抽象度の高い入力の層（easy）だけは global な関数を持つ

- 状態: 採用
- 決定者: user（`play("cde")` で鳴る top level 関数の層を作ること・global な関数を持つ snippet 群を新しい方針として認めること・`play` は1回鳴らし loop は `playLoop` に分けること・引数が複数なら複数 track にすること・`play`/`playLoop` の引数を track 0 から順に割り当てること・track を状態で持たず（`selectTrack` は作らない）、`changePatch`/`changeEffect` は引数で track を受けること・`play` で引数に入らない track を鳴らさないこと・browser で即座に変更して戻さない `changePatch`/`changeEffect` を作ること・patch も effect も cell を書く版と書かない版を揃え、easy は書かない版にして画面の記述量を最小にすること・定義の snippet は使える関数名を最後の値で console に出し、`easy-<機能>` を1機能ずつの案内にして定義の cell を持たせないこと・`easy-<機能>` は冪等にせず関数呼び出し1行にし、`try-` の冪等性の規則から外すために名前を `easy-` にして `try-` より前に並べること・定義の snippet を `easy-init` として先頭に置くこと・easy を始めると全 track の再生・clip・plugin が消えて楽器が前置きの patch から始まること・それと同じ初期化を global な `reset` で呼べること）。`reset` の名前、track の数を変えないこと、楽器の無い track にだけ patch を読むことは AI の提案を user が承認。既定値を前置きの cell の引数に置くこと・既定値を `create` の closure に閉じ込めること・effect の置き場所を cat-smf-editor と同じ規則にすることは AI の提案を user が承認。effect の cell の形・置き換えで引き継ぐ bus は AI の設計
- 関連: [0001](0001-patch-browse-by-plugin-state.md)（browse と cell）、[0002](0002-cells-address-instruments-by-track.md)（cell は track 番号で話し「今の track」を作らない）

## 決定

- snippet を2層に分ける。従来の層は ADR 0002 と AGENTS.md の snippets 節のまま（cell を `{ ... }` で包み、track は説明変数で渡し、「今の track」の global を作らない）。easy の層だけが global な関数を持つ。easy の層も track の状態は持たない。
- easy の層は `lib/easy.js` の `create({format, plugin, patch, bpm})` が返す `play`・`playLoop`・`reset`・`changePatch`・`changeEffect` を、前置きの cell（`easy-init`）が `Object.assign(globalThis, ...)` で global に出し、続けて `reset()` する。plugin・patch・tempo の既定値は前置きの cell に書き、lib には書かない。
- `reset()` は全 track の再生・loop・clip・plugin（楽器も effect も）を消す。track の数は変えない。
- `play(...inputs)`・`playLoop(...inputs)` は i 番目の引数を track i に置き、引数に入らない track は鳴らさない。`changePatch(track)`/`changeEffect(track)` は引数の track（0 が先頭）で browser を開く。楽器の無い track にだけ `ensureInstrument` で前置きの plugin を patch つきで置き、楽器のある track の patch には触れない。
- patch・effect の browser は `repl.patchBrowser.open({track, kind, cell})` の1本で開く。`kind` は `'patch'`（既定）か `'effect'`、`cell:false` なら最後の選択を cell にしない。`lib/patch.js` と `lib/effect.js` の `change({track, cell})` は選ぶたびに track へ適用し、閉じても戻さない。easy の `changePatch`/`changeEffect` は `cell:false` で呼ぶ。
- snippet は `easy-init` が関数を定義して関数名の文字列を返し、`easy-play` などの `easy-<機能>` が関数呼び出し1行の cell 1つで1機能ずつ使い方を見せる。`easy-<機能>` は冪等にせず、上に並ぶ snippet を先に選んだ状態（定義済み・loop 中）に頼る。一覧は `easy-`、`try-`、その他の順に並ぶ。
- effect の置き場所は cat-smf-editor と同じ: 同じ plugin の effect があればそれに state を読ませ、無ければ track の最後の effect（output へつながる楽器以外の plugin）を置き換え、effect が無ければ output の手前に足す。置き換えでは古い effect の bus 0 の接続だけを引き継ぐ。
- effect の cell は `effect.js` の `use({format, plugin, preset, value, track})` にする。`preset` は preset ファイルの plugin の root からの相対パス（内蔵 preset の plugin では root 自体で空文字）、`value` は player の catalog がそのファイルの中で preset を選ぶ値。`repl.effectState` がこれを player の `--restore-state` に渡して state にする。

## 理由

- ADR 0002 が「今の track」の global を退けた理由（cell の結果が実行順と履歴で変わり、画面に見えない）は easy の層でも成り立つ。それでも user は、`play("cde")` だけで鳴る記述量の少なさを選んだ。global に出すのを関数だけにして track の状態を持たず、従来の層には持ち込まないことで、その代償を easy の層だけに閉じる。
- 引数を track 0 から順に割り当てる形は、引数が1つのときと複数のときで規則が1本で済む。
- track の状態を持たないのは、対象 track を状態で持ち `selectTrack` で変える形を検証した結果、そのほうが使いやすいと user が判断したから。
- user が easy の層に求めたのは1行で済む短さで、`try-` の冪等性（何度選んでも同じ状態）とは両立しない。前提を作る cell を各 snippet に入れると1行でなくなるので、前提は一覧の並び順に持たせ、名前で `try-` と分けて規則を分ける。
- easy を始めたら全部まっさらになる、が user の想定だった。`play` が楽器のある track の patch に触れないので、`changePatch` で選んだ patch は `reset` するまで残る。
- 既定値を前置きの cell に書けば、AGENTS.md の「lib に plugin・tempo を決め打ちしない」を守ったまま、user が cell を直すだけで既定値を変えられる。
- browser の開き方を `kind` と `cell` の引数にすれば、Rust 側は player への `Open` の種類と cell の有無を変えるだけで済み、適用の仕方（試聴して戻す・即座に変える・patch・effect）は lib に書ける。
- player の `--patch-roots` は effect の plugin（Surge XT Effects・Dragonfly）の root も返し、`--restore-state` は effect の参照も state にする。root からの相対パスで書けば cell にマシン固有の絶対パスが入らない。
- Surge XT → Surge XT Effects は bus 0 と bus 1 の2本でつながる。Dragonfly のように bus 1 の無い plugin へ置き換えると、bus 1 の接続を張り直せずに失敗した。

## 却下した案

- **`easy-<機能>` のそれぞれに定義の cell を入れる。** 単独で選んでも動くが、定義の cell が毎回貼られて記述量が増え、定義のたびに全部が reset される。関数名を console に出せるのも最後の値が定義の snippet だけになる。
- **`try-easy-<機能>` の名前のまま冪等にする（`changePatch(0)` の前に `playLoop` の cell を足す）。** 1行でなくなり、user の求めた短さと逆になる。
- **`try-easy` の名前のまま定義を残す。** `easy-` が `try-` より前に並ぶので、定義がそれを使う snippet より下に来る。
- **初期化で plugin を残す。** user の想定（easy を始めたら patch は Init Saw、effect も消える）と違う。
- **関数名を `init` にする。** `lib/tutorial.js` の `init(options)` と名前がかぶり、意味も違う。
- **`var play = ...` を前置きの cell に直接書く。** 既定値の置き場所が global に別々に増える。closure に閉じれば global に出るのは関数名だけになる。
- **init.js で easy の関数を起動時から生やす。** 前置きの cell を1回実行すれば済み、全 E2E の起動経路を変えずに済む。
- **対象 track を状態で持ち `selectTrack` で変える。** 検証した結果、track を引数で渡すほうが使いやすいと user が判断した。
- **effect の cell に player の catalog の表示名を書く。** repl には effect の catalog が無く、表示名から preset ファイルへ戻す手段が無い。

## 結果

- easy の層の cell は、上の cell や上に並ぶ snippet の実行結果（前置き・loop）に依存する。どの関数の行き先も、その cell の引数だけで決まる。前置きを実行していない状態では `play` が未定義のエラーになり、loop していない状態で `changePatch(0)` を選ぶと鳴らないまま browser が開く。
- `reset()` は plugin を `dispose` した後に host の待ち行列を処理させる。処理させないと、同じ cell の続きでは消した plugin がまだ track に残って見える。
- `changeEffect` で同じ plugin の preset を選び直すと、最後の effect ではなく同じ plugin の effect が変わる。
- Surge XT Effects の cell は `preset` と `value` に同じ相対パスが並ぶ（その plugin では player の値が相対パスそのものだから）。
- effect の置き換えの間、uapmd は plugin の作成・削除のために transport を止め、loop の音が一瞬途切れる（ADR 0002 の楽器の置き換えと同じ）。
