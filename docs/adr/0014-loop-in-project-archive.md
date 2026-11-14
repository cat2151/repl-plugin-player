# ADR 0014: 音符は標準clipに統一し、loopは演奏中の設定にする

- 状態: 採用（独自loop planとarchive拡張による保存の決定を撤回）
- 決定者: user（UX検証の結果、独自loop仕様の保存と通常timelineからの分離を不要と判断。更新は周回境界を待たずuapmdの仕組みを使う）
- 関連: [0012](0012-easy-layer-with-global-state.md)（`play`と`playLoop`の入口）

## 決定と理由

当初、AIは演奏中の編集を周回境界でまとめて反映するため、通常clipとは別のloop planを採用した。その音符がproject保存から漏れたため、音源と一緒に保持できるarchive拡張へ独自データを保存する方針を選んだ。

userがUX検証した結果、別アプリでprojectを読むとloopの音符が標準clipとして扱えず、今回の問題が見つかった。userが必要とするのは同じtimelineを演奏中に指定範囲で反復することで、loop設定の保存も境界での更新予約も要件ではなかった。uapmd自体に通常clipを範囲反復する処理があるため、独自の演奏データと保存形式を持つ理由がなくなった。

音符はloopの有無によらずuapmd標準のtimeline clipに統一し、音源・設定とともに標準project archiveへ保存する。loopの有効・範囲はセッション内のtransport設定だけとし、project読込時は無効にする。編集はuapmdの通常のclip更新経路を使い、アプリ独自の周回境界待ちや演奏スケジューラを設けない。JSに不足する範囲設定だけを薄く公開する。

## 結果

`play`と`playLoop`は記述用の入口として残すが、音符・表示・保存のモデルは共通になる。反復を解除してもclipは残る。独自loop拡張の読み書きと再生補修は撤去する。既存の試用projectはuserの指示で破棄し、旧独自形式の互換読込や移行は作らない。
