# その他
- AIはREADME.mdとREADME.ja.mdに追記しないこと。理由: README.mdとREADME.ja.mdは人間向けで、機能の挙動はコード・help・ADRが持つ
- ビルドロック回避を含め、`--target-dir`等で既定以外のtargetディレクトリを作成・使用することを禁止
- 互換性不要、まだこのアプリはuserが作者一人のプロトタイピング段階である
- もしrelease build時にビルド対象アプリが起動中のせいでrelease buildが落ちたら、アプリをkillしてrelease buildをしなおすこと

# ビルドエラー時の変更境界
- ビルドエラーが出たら、ソースやビルド設定を変更する前に、エラーログ、差分、既存のビルド手順を読み取りで確認し、原因を切り分ける。CMakeCache等のGit管理外の設定と、生成済み依存ソースも対象とする。ビルドを通すことだけを理由に、根拠のない変更や検証の迂回を行わない。
- 変更前に、原因を直す根拠、変更対象、元の依頼範囲内かを簡潔に記録する。今回の実装が直接生じさせた通常のコンパイルエラーの修正は範囲内とする。元の依頼に含まれないビルド方式・依存関係・ビルド設定の変更が必要なら、原因、変更案、再現性への影響をuserに提示し、合意前に変更しない。既に合意済みの範囲は聞き直さない。
- 設定変更を試す場合は変更前の値と撤回方法を控え、失敗した変更は次の試行前に戻す。新たな根拠なしに別の変更を重ねない。生成済み依存ソースを戻す場合は、その差分が既存のビルド手順によるものか確認し、由来不明の変更は上書きしない。
- 同じビルド領域・依存キャッシュを変更する操作は直列に実行する。cargo、CMake、依存ソースの復元、キャッシュ編集を開始する前に、自分とsubagentの先行操作が完了していることを確認する。
- 完了報告前に試行用設定と失敗残骸を確認し、不要なものを除去する。報告では実際に成功した方法、検証範囲、未解決の原因を区別し、既存キャッシュでの成功を新規環境での再現性確認として扱わない。

# 依存ライブラリへのローカル補修
- 補修の導入前に、目的、適用場所、依存checkoutを書き換えるか、再clone・既存キャッシュ・上流更新時の挙動と再現性への影響をuserへ説明する。変更範囲の合意が必要な場合は先に得る。合意済みの範囲は聞き直さない。
- UAPMDへの補修は `patches/uapmd/<補修名>/` に処理、回帰テスト、説明書、作者向け報告文をまとめる。生成ソースは既定の `target/shim` 内に置き、依存checkoutを直接書き換えない。
- 説明書には原因、適用条件、検証した上流リビジョン、再適用の仕組み、適用失敗時の調査手順、上流対応後の撤去条件、作者への報告状態（未報告／報告済み・リンク）を残す。報告文の作成を外部送信の許可として扱わない。
- 補修は適用条件を確認し、失敗時には補修名、対象ファイル、失敗条件、上流リビジョン（取得可能な場合）、説明書の場所を明示して停止する。黙ってスキップしたり、ビルドを通すために検証を迂回しない。上流対応済みか確認してから更新・撤去する。
- 現在の補修一覧: [CLAP Note形式](patches/uapmd/clap-note-dialect/README.md)、[CLAP state読込失敗](patches/uapmd/clap-state-load-error/README.md)、[PluginScanToolのdata directory競合](patches/uapmd/scan-tool-data-dir-race/README.md)、[stb revision固定](patches/uapmd/stb-revision-pin/README.md)、[CLAP/VST3 parameter automation](patches/uapmd/parameter-automation/README.md)、[プラグイン入力ノート観測](patches/uapmd/note-monitor/integration.md)、[Rust script API接続](patches/uapmd/script-api/README.md)、[CLAP preset列挙・読込](patches/uapmd/clap-presets/README.md)、[CLAP parameter metadata](patches/uapmd/parameter-metadata/README.md)、[共通周期loop](patches/uapmd/loop-playback/README.md)。新規追加・撤去時は一覧も更新する。

# rustソースを変更して作業完了報告時
- 450行をoverした*.rsは、単一責任の原則に従いファイル分割
- cargoのclippyとfmtを使うこと
- リリースビルド（ cargo build --release ）をすること

# Codexへ
- Computer useを使う前に、「scriptを生成してCLI実行可能か？」を検討し、可能ならCLIを選ぶこと

# 画面の文字
- 「～を押すと～できます」の操作説明や「今は～の状態です」の状態表示を、画面に文字で出さない。状態は色・枠など文字以外で示す。必要になったら user が表示を指示する。理由: 冗長な文字列は毎回だいたい邪魔になる。

# snippets
- `snippets/builtin/` の snippet は全部、user が常に手直しする前提の内容（音符・plugin・preset・parameter値・名前・並び）である。AIが書いた内容を正としてテストで固定しない。snippet を使うテストが見るのは「実行してエラーにならないか」だけにし、音高・音長・音色・名前などを期待値にしない。理由: AIが選ぶ内容は美的に使えず、user が直すたびにテストが落ちると改善の邪魔になる。
- テストが特定の内容を必要とするときは、snippet を読まずにテスト専用の script を持つ。理由: snippet を書き換えてもテストが壊れないようにする。
- `snippets/builtin/lib/` には plugin・preset・effect・parameter値・tempo・音符などの決め打ちを置かず、呼出し側の snippet が引数で渡す。理由: lib は snippet から再利用される部品であり、内容の選択は snippet 側に属する。

# API追加
- 追加するuapmd APIや、repl APIは、極力薄いwrapを小さく作るだけ（どうしてもそれがないと動かない or 遅すぎて音が途切れてしまう というときだけ作る）にして、基本的にはscript側の記述で組み立てる。

# MML
- MMLやコード進行は基本的に `>cg<cg` や `Isus4-I` を書く。これらはloopで使っても適切である
- デフォルトのt120 l8 o5 はMMLに書かない。長い音符が必要なときだけl4等を書く。octave upしたいときだけ `<` を書く、octave downしたいときだけ `>` を書く
- show UIやパラメーター変更UIのsnippetsを書くときは、演奏をloopさせる
