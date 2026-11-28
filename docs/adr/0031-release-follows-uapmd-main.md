# ADR 0031: Release の CI は uapmd の main を追い、版を固定しない

- 状態: 採用
- 決定者: user（Release の CI が uapmd の main を追い、版を固定しないこと。main が進んで落ちたら、そのとき user が対応すること）
- 関連: [0029](0029-uapmd-host-built-by-build-rs.md)（補修を含む UAPMD host の build）、`.github/workflows/release.yml`

## 決定

- Release の CI（`.github/workflows/release.yml`）は、その日の `atsushieno/uapmd` の main の commit を取り、それで build する。
- `UAPMD_VERIFIED_SHA` は build する版を決めない。手元で確かめた uapmd の commit を記録し、リリースノートに一致・不一致を出すためだけに使う。

## 理由

- user が uapmd の最新機能を取り込むことを優先した。落ちたときは user がそこで対応すればよい。

## 却下した案

- **`UAPMD_VERIFIED_SHA` に固定して build する。** 上流が進んでも Release は落ちないが、uapmd の新しい修正が、誰かが版を上げるまで Release に入らない。

## 結果

- uapmd の main が補修の当たる所（anchor・baseline）を変えるたびに、その日の Release の build は補修の適用失敗で止まる。
- build が落ちると publish は走らないので、前の Release が残る（publish job は `needs: [resolve, build]`）。
- 対応するときは、補修を新しい上流に合わせ、`UAPMD_VERIFIED_SHA` を上げる。
