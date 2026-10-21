# リポジトリ構成と依存方向

## 配置と責務

| 配置                              | 役割                                 | 配布との関係                         |
| --------------------------------- | ------------------------------------ | ------------------------------------ |
| crates/asearch-rs/                | Rustの照合コア                       | ネイティブとWASMで共通               |
| crates/asearch-wasm/              | wasm-bindgenの薄いFFI境界            | WASMビルドの入口。デモのRustではない |
| crates/asearch-cli/               | 索引構築・更新・検索・検証のCLI      | コアを呼ぶCLI。WASMの依存ではない    |
| packages/asearch-wasm/src/        | JS/TS公開APIと環境別の初期化         | ラッパをビルドして配布               |
| packages/asearch-wasm/dist/       | WASM・生成グルー・JS・型・ライセンス | 生成物。Git追跡対象外                |
| crates/*/tests/                   | Rustの統合テスト                     | 配布する照合コードから分離           |
| packages/asearch-wasm/test/       | JS単体・型・tarballテスト            | npm配布対象外                        |
| tests/browser/                    | ブラウザ統合テストとそのハーネス     | npm配布対象外                        |
| crates/asearch-rs/tests/fixtures/ | 生成済みの互換性データ               | 製品Rustの通常ビルドからは利用しない |
| tools/                            | ビルド・検証・期待値生成のツール     | 製品runtimeからは利用しない          |
| fuzz/                             | ASIXローダのlibFuzzer/ASan検証       | 独立workspace・nightly。製品依存外   |
| vendor/upstream/                  | 上流の原本とライセンス               | oracle・帰属表示に使用               |
| docs/                             | 設計・仕様・開発計画                 | 現行契約と将来計画を区別             |

`tests/browser/fixtures`はテスト専用です。GitHub Pages用の検索UI・データセット・配信設定は別repo [asearch-demo](https://github.com/tkhshtsh0917/asearch-demo)へ配置します。Workerによる検索・説明・差分更新のUIとPagesワークフローを実装し、ローカルの3エンジンとモバイルプロファイルで検証しています。hosted CI/Pagesはpush後の確認事項です。本repoはRustコア・CLI・WASM・薄いTypeScript APIと検証を持ちます。デモは版とハッシュを固定した本体の配布成果物を利用し、最新branchをビルドのたびに取り込みません。npm公開は必須にせず、検証済みtarballでも受け渡せる構成にします。AutoWikiQAの質問文・帰属表示・データ用ライセンスはデモrepoが管理し、本体の依存関係には加えません。

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

## 分割後に維持する境界

本体側の`tests/browser`は初期化・公開API・WASM境界の回帰テスト、デモ側の`tests`は検索画面・IME・更新操作のE2Eです。両方でブラウザを使いますが検証責務は異なり、移動や一本化はしません。本体の合成ベンチ・独立oracle・上流コピーも、製品を単独検証するため本体に保持します。

デモ側のコーパス取得、ASIX配信資産の組み立て、帰属表示はデモに閉じています。ASIXの構築・照合・正規化・更新の意味論は本体の公開APIを利用します。各repoのライセンス表示と、配布単位ごとの第三者NOTICEは、同じ内容でも再配布条件を満たすため保持します。
