# コントリビューション

[English](CONTRIBUTING.md)

## 範囲

このリポジトリはRustの照合、ASIXの検証と生成、ネイティブ解析、CLI、WASMの公開境界を担当します。JS/TSバインディング、ブラウザテスト、互換モデルは[デモのintegrationワークスペース](https://github.com/tkhshtsh0917/asearch-demo/tree/main/integration)が担当します。[README](README.ja.md)と[アーキテクチャ](docs/ja/architecture.md)から読んでください。

## 開発時の検証

rustupを導入すると、`rust-toolchain.toml`がコンパイラー・コンポーネント・WASMターゲットを選びます。リポジトリのルートで実行します。

```sh
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo clippy --locked -p asearch-wasm --target wasm32-unknown-unknown -- -D warnings
cargo test --locked --workspace
cargo doc --workspace --no-deps
```

[fuzz検証](fuzz/README.ja.md)には別に固定したnightlyツールチェーンを使います。ビルド手順はREADMEにあり、製品のビルドにNode.jsは不要です。

## 変更と回帰検証

各変更の目的を絞り、関数・ファイルを責務ごとに分け、意味のある処理のまとまりの間に空行を入れます。コメントには処理内容と不可欠な制約を書きます。製品のRustにはunsafeコードを使いません。依存を固定し、生成物はGitへ含めません。上流のライセンスと帰属表示を保ち、秘密値や作業端末固有のパスを含めないでください。

動作の修正には回帰検証を加えます。アルゴリズムを変える場合は独立した照合・整列モデルを使い、同じ最適化の別実装だけを独立oracleと見なしません。互換性の変更を明示しない限り、ASIX v1、正規化の記述子、資源上限、完全な候補履歴、結果の寿命、原子的な採用を保ちます。正常入力に加え、失敗後の回復も検証します。

性能の変更では計測前に結果の同等性を確認し、同じツールチェーンで両方の成果物を準備し、ペア比較と別の確認計測を行います。入力・成果物・ソースの識別情報と悪化も記録し、ばらつく共有CIの時間を速度保証にしません。[性能上の判断](docs/ja/performance.md)を参照してください。

## ドキュメント

READMEと仕様書には対応する動作と使い方を書きます。このガイドとテスト・ツールのガイドには開発と検証を書きます。英語原文と日本語訳のコマンド・制約・図・例を揃えてください。動作を変える場合は関連する契約を更新し、対象は[ドキュメント一覧](docs/ja/README.md)から探せます。

## 問題の報告

バージョン、最小の入力、期待した動作と実際の動作、関連する再現コマンドを含めてください。脆弱性の疑いには[セキュリティ方針](SECURITY.md)を使い、機微情報を公開しないでください。
