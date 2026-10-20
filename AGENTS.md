# Asearch — contributor instructions

## 現在の範囲と参照先

- Rustコアは`crates/asearch-rs`、薄いWASM境界は`crates/asearch-wasm`、JS/TSラッパは`packages/asearch-wasm`。
- ネイティブCLIは`crates/asearch-cli`。現在はfixture検証とnative計測を持ち、製品WASMはCLIや計測コードへ依存しない。
- M0互換照合、M1のmatchLevel/free/dispose・計測基盤・Ideal・汎用ScalarWm/Workspaceを実装済み。M1はローカル受入完了。M2はRust正規化・ポリシー・QueryWorkspace・TransposedEngine・単一文書索引・ASIX base保存/ロード・CLI build/searchを実装。JS一括API・stream・規模計測は未実装。後続APIをstubとして先取りしない。実装順・受入条件は[開発計画](docs/PLAN.md)。
- 規範は[仕様索引](docs/spec/README.md)。節番号は文書をまたいで共通。M0は[§19](docs/spec/compatibility.md)を優先し、[§2](docs/spec/semantics.md)の意味論に従う。
- 着手前に対象節を読む。M1: §5/12/15、M2: §3/4/8–11、M3: §4.7/6/10/12、M4: §11/20、M5: §8/9、M6: §7、M7: §4.6/5/16。共通の受入表は§14.1。
- 決定の理由は[DECISIONS](docs/DECISIONS.md)。仕様が曖昧なら同梱上流・参照実装で確かめ、未決なら選択肢と理由を記録して質問する。
- 人向けの導入・ビルド・検証手順は[README](README.md)へ集約する。文書編集前に対象見出しを確認する。

- 構成と依存方向は[リポジトリ構成](docs/repository-layout.md)。ブラウザ検証は`tests/browser`、GitHub Pages用UIデモは別repo`asearch-demo`へ配置する方針。製品はデモ・ハーネスへ依存しない。

## 意味論と互換性

- 互換の正は`vendor/upstream/asearch.ts/lib/index.js`（1.0.3）に定義済みの意図的逸脱を加えたもの。既定AsearchTsはtextをcode pointで走査し、AsearchTsLibは公開物のUTF-16挙動を再現する。patternはcode point、長さ上限は31 UTF-16単位。
- 非単調なlevel、32以上のシフト折り返し、Stringオブジェクト、生のmode、signed masks、JS casingを保持する。初期化・例外順・受理範囲は§19に従う。変更は仕様とテストを同時に更新する。
- 将来の一括検索はIdeal意味論。UI/DOM/IME制御、言語資源、配信基盤は範囲外。更新はサーバー生成のbase/delta/tombstoneを読む。クライアント編集層は作らない。

## 安全性と性能

- Rustコアは`#![forbid(unsafe_code)]`。想定入力の失敗をResultで扱う。通常のRust照合level 0〜31は確保ゼロを計数アロケータで検査する。JS/WASM境界の確保とは区別する。
- ブラウザへ届くコードはprocess等のNode専用globalに依存しない。孤立サロゲートを保つためUTF-16で渡す。65,536要素のmask表を呼出しごとに初期化しない。
- 将来の索引ローダは信頼しない入力を検証し、予算と失敗時の旧状態保持を守る。SIMDは別成果物で機能検出し、relaxed-simdは使わない。
- 速度の主張は結果が一致する製品同士の計測でのみ行う。代理実装の数値を製品へ転用しない。性能変更は§15のpaired run、環境・版・標本数・p50/p95/p99を添える。

## 品質と公開

- 必須コマンドはREADME。Rustはrustfmt/Clippy/test、JS/TSはBiome/tsc/node:test、文書はPrettierとリンク検査、ブラウザはPlaywright。
- 上流はbyte-for-byteで保持し、SHA256SUMSを確認する。生成済みfixtureは固定seed/localeで再生成checkを行い、更新時は生成コマンドと差分理由を示す。
- `tools/reference`は独立した正しさのモデルであり、製品実装でも製品性能の証拠でもない。Node/Pythonの検証結果をRust製品テスト件数と混ぜない。
- ツール版はlockfile、Rustはrust-toolchain.toml、Node/pnpmはmise.tomlで固定する。外部の版・ハッシュ・数値を推測で書かない。
- 関数・クラス・ファイルは責務で分け、入力検証・変換・受け渡し等の意味の切れ目へ空行を入れる。原本や生成物を一括整形しない。
- 公開ファイルと配布物に秘密情報、端末の絶対パス、非公開作業環境への参照を入れない。WASMにはコンパイル時のパスをremapする。
- コミット・pushは利用者が明示的に依頼した場合にのみ行う。distやローカルログを追跡しない。
- 報告は実装/未実装、件数と失敗、仕様逸脱、未検証事項、次の段階を分ける。実行していないCI/ブラウザ/性能を成功扱いしない。
