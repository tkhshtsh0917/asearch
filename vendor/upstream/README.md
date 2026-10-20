# docs/upstream — 移植元の同梱コピー（互換の正、ネットワーク無しでも使える）

M0 の互換テスト（フィクスチャ生成、asearch.ts のテストと README 例の移植）に使う、移植元のファイルのコピー。**中身を編集しない。** 各ファイルは元のライセンスのまま（下表）。プロジェクト本体のライセンス（MIT OR Apache-2.0）は適用されない。

このディレクトリは `node_modules` と同じ形（`asearch.ts/`、`asearch/`）なので、参照スクリプトにそのまま渡せる:

```bash
(cd docs/upstream && sha256sum -c SHA256SUMS)                # まず改変が無いことを確かめる（パスはこのディレクトリ基準）
node docs/reference/oracle.js docs/upstream < cases.json      # npm 実物と同じ挙動
python3 -I docs/reference/verify.py docs/upstream 1 3000
node docs/reference/check_lib_quirks.js docs/upstream/asearch.ts/lib/index.js
```

## 取得元（2026-10-19 15:43 UTC ＝ 2026-10-20 00:43 JST に取得）

| ディレクトリ | 取得元 | 版・コミット | ライセンス（宣言） |
|---|---|---|---|
| `asearch.ts/` | npm `asearch.ts@1.0.3` の tarball（`npm pack`）。`README-ja.md` だけは GitHub `teramotodaiki/asearch.ts` から | npm integrity `sha512-rT4lvTYWlpDy4tC88v2/oYJ67+YjAyXpKFYwfcggtMO/N6xSufpe1736Da1ruCfyrH6xWwXpztoCTidphxxV8Q==`。GitHub HEAD `0349ddbe11a783fd44239b559e33b0861e6294c4`（2026-10-15、依存更新のコミット） | ISC（package.json。LICENSE ファイルは無い。author: teramotodaiki） |
| `asearch/` | npm `asearch@1.0.1`（shokai/node-asearch）の tarball。`tests/test.js` だけは GitHub から | npm integrity `sha512-pR3rszVCSkoS2HhQJVzycQ6H1R1R9JOS0FyfKP8dY4Xi4zSb9CbMpUUsMP1fL8dQNEucXiEOvleGis81QVhviQ==`。GitHub HEAD `ad8e7412b443a52a9f4ef5c9a8de753a6aa29aea`（2021-05-23） | BSD-2-Clause（package.json。LICENSE ファイルは無い。author: Sho Hashimoto） |
| `asearch-ruby/` | GitHub `masui/asearch-ruby` の `README.md` だけ | HEAD `b554d5a444751ab1b50aa2b9a6fb582179f4fe7f`（2014-01-05） | README に MIT の全文（著作権者欄は "FIXME full name"）。ただしソース `lib/asearch.rb` の先頭は "Copyright 2011 Pitecan Systems. All rights reserved." で、README と食い違う。**本リポジトリは asearch-ruby のコードを取り込まない**（移植元は asearch.ts）ので、ソースは同梱しない |
| `licenses/` | `LICENSE-APACHE` は crates.io の `wasm-bindgen-0.2.129` 同梱のもの（Rust で一般的な Apache-2.0 の全文）。`MIT.txt`／`ISC.txt`／`BSD-2-Clause.txt` は npm `spdx-license-list@6.12.0` の `spdx-full.json` の licenseText | — | 各ライセンス文そのもの。`<year> <copyright holders>` などの欄はテンプレートのまま（ISC.txt の著作権行は ISC 自身のもの） |

integrity は `npm view <pkg>@<ver> dist.integrity`、コミットは `git rev-parse HEAD` の出力をそのまま転記した。

## 確認済みのこと（2026-10-20 00:43〜00:45 JST）
- npm の最新版は今も `asearch.ts@1.0.3`（2023-12-11 公開）。GitHub の master の `src/index.ts`、`src/index.test.ts`、`README.md` は 1.0.3 の tarball と同一（`diff` で差分なし）。
- node-asearch の `index.js` は npm の tarball と GitHub の HEAD で同一（`cmp`）。
- この同梱コピーを `verify.py` に渡すと、レプリカとの不一致 0（シード 1、10,023 ペア × level 0..5）。`check_lib_quirks.js` も SPEC §2.5 の癖をそのまま再現した。

## 注意
- 互換の正は `asearch.ts/lib/index.js`（公開物）。`src/index.ts` は意図の確認用（SPEC §2.4）。
- `asearch.ts/src/index.test.ts` は ava ＋ TypeScript 用のテスト。M0 では、アサーションの中身を変えずに本リポジトリのテストランナーへ移植する（SPEC §19.7）。
- `NOTICE` には上の 3 つの出典とライセンスの宣言を書く（SPEC §19.10）。上流に著作権行が無いものに、年や著作権者を推測で書き足さない（package.json の author 欄を「author」として書くのは可）。
- プロジェクト自身の `LICENSE-MIT` の著作権者と年は利用者が決める（`docs/HANDOFF.md`）。
