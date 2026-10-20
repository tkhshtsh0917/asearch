# リポジトリ構成と依存方向

## 配置と責務

| 配置                              | 役割                                 | 配布との関係                         |
| --------------------------------- | ------------------------------------ | ------------------------------------ |
| crates/asearch-rs/                | Rustの照合コア                       | ネイティブとWASMで共通               |
| crates/asearch-wasm/              | wasm-bindgenの薄いFFI境界            | WASMビルドの入口。デモのRustではない |
| crates/asearch-cli/               | ネイティブの検証・計測コマンド       | コアを呼ぶCLI。WASMの依存ではない    |
| packages/asearch-wasm/src/        | JS/TS公開APIと環境別の初期化         | ラッパをビルドして配布               |
| packages/asearch-wasm/dist/       | WASM・生成グルー・JS・型・ライセンス | 生成物。Git追跡対象外                |
| crates/*/tests/                   | Rustの統合テスト                     | 配布する照合コードから分離           |
| packages/asearch-wasm/test/       | JS単体・型・tarballテスト            | npm配布対象外                        |
| tests/browser/                    | ブラウザ統合テストとそのハーネス     | npm配布対象外                        |
| crates/asearch-rs/tests/fixtures/ | 生成済みの互換性データ               | 製品Rustの通常ビルドからは利用しない |
| tools/                            | ビルド・検証・期待値生成のツール     | 製品runtimeからは利用しない          |
| vendor/upstream/                  | 上流の原本とライセンス               | oracle・帰属表示に使用               |
| docs/                             | 設計・仕様・開発計画                 | 現行契約と将来計画を区別             |

現在UIデモはありません。`tests/browser/fixtures`はテスト専用です。GitHub Pages用の検索UI・データセット・配信設定は別repo `asearch-demo`へ配置する方針です。本repoはRustコア・CLI・WASM・薄いTypeScript APIと検証を持ちます。デモは版とハッシュを固定した本体の配布成果物を利用し、最新branchをビルドのたびに取り込みません。npm公開は必須にせず、検証済みtarballでも受け渡せる構成にします。デモrepoの新設・公開は未実施です。

## 依存方向

```text
asearch-demo（別repo） / browser・package tests
                      ↓ 公開APIを利用
packages/asearch-wasm（JSラッパ）
                      ↓ 生成グルー経由
crates/asearch-wasm（FFI境界）
                      ↓
crates/asearch-rs（照合コア）
```

製品はasearch-demoやブラウザテストへ依存しません。デモ用の機能やUI処理をRustコアへ入れません。利用例もテストも本体と同じビルド済みライブラリを使い、別の検索エンジンを持ちません。配布検査ではnpm tarballにソース・Rust・テスト・利用例が混入していないことを確認します。

## 構成の根拠

単一のWASMリポジトリ標準があるわけではありません。本プロジェクトでは、[Cargoのsrc/tests/examples規約](https://doc.rust-lang.org/cargo/guide/project-layout.html)、[AutomergeのRust実装とJS層の分離](https://github.com/automerge/automerge#repository-organisation)、[wasm-bindgenの独立したexamples](https://wasm-bindgen.github.io/wasm-bindgen/examples/index.html)を参考に、責務と依存方向を優先しています。特定のデモ用バンドラやフレームワークを製品へ必須化しません。
