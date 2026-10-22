# asearch

[原文](README.md) · 日本語版

Rustによるワイルドカード付き曖昧照合ライブラリです。一括索引、日本語正規化、逐次検索、base/deltaの原子的な更新、意図表現のDAG検索を提供します。

## 構成

- `crates/asearch-index`: ASIX v1の形式定義・検証・共通正規化。
- `crates/asearch-builder`: サーバー向けのbase/delta生成とcanonicalシリアライズ。
- `crates/asearch-engine`: ロード、内部検索構造の構築、検索、差分採用、説明。
- `crates/asearch-cli`: 索引の構築・検索・差分生成・圧縮・manifest生成。
- `crates/asearch-wasm`: コアをWebAssemblyへ公開するRust境界。
- `fuzz`: 索引形式と意図表現の入力検証。

TS/JSラッパ、バインディング生成、ブラウザテスト、検索デモは[asearch-demo](https://github.com/tkhshtsh0917/asearch-demo)が管理します。このrepoのビルドにNode.jsは不要です。

## 設計ドキュメント

- [アーキテクチャ](docs/ja/architecture.md): サーバー・CDN・ブラウザでのcrateの連携をSVGで示します。
- [Bitapの流れ](docs/ja/algorithm.md): ビット演算の流れをGIFと数値例で説明します。
- [ASIX v1のバイト配置](docs/ja/asix-format.md): バイト位置ごとの仕様、配置図、CLIで再現できる実例をまとめます。

[日本語ガイド](docs/ja/README.md)から文書一覧へ進めます。

## ビルドと検証

Gitとrustupを用意し、ルートで実行してください。Rust版とWASMターゲットは`rust-toolchain.toml`に固定しています。

```sh
cargo build --locked --workspace
cargo build --locked -p asearch-wasm --target wasm32-unknown-unknown --release
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo doc --workspace --no-deps
```

WASMは読み取り・検索専用で、asearch-builderへ依存しません。生成・保存はCLIまたはasearch-builderを利用します。native engineの既定builder featureは従来のRust保存APIを維持するための任意機能です。

WASMの出力は`target/wasm32-unknown-unknown/release/asearch_wasm.wasm`です。ブラウザ向けのグルー生成・最適化・JS APIはデモrepoの手順に従ってください。

## Rustでの利用

```rust
use asearch_engine::{Asearch, MatchMode};
fn main() -> Result<(), asearch_engine::PatternTooLong> {
    let pattern = Asearch::new("hello", MatchMode::Exact)?;
    assert!(pattern.is_match("helo", 1));
    Ok(())
}
```

`crates/asearch-engine`をpath依存として利用できます。索引やWorkspaceのAPIは`cargo doc`で参照できます。

```sh
cargo run --locked -p asearch-cli -- build docs.jsonl questions.asix
cargo run --locked -p asearch-cli -- search questions.asix 検索語
cargo run --locked -p asearch-cli -- delta changes.jsonl delta.asix
cargo run --locked -p asearch-cli -- compact compact.asix questions.asix delta.asix
cargo run --locked -p asearch-cli -- manifest faq 1 manifest.json base.asix=questions.asix
```

build入力は1行1文書のJSONLです。例: `{"key":1,"texts":["利用方法"]}`。delta入力は`{"op":"upsert","key":1,"texts":["新しい内容"]}`または`{"op":"remove","key":1}`。出力は新規作成します。配信と世代の管理は呼出側の責務です。

## 謝辞と互換性

[asearch.ts](https://github.com/teramotodaiki/asearch.ts)から着想を得て、1.0.3との互換性検証を出発点にしています。`AsearchTs`はcode point、`AsearchTsLib`はUTF-16単位の照合を提供し、`Ideal`は単調な編集距離の意味論を提供します。互換profileのlevelは単調とは限りません。仕様と互換oracleは[デモrepoのintegration](https://github.com/tkhshtsh0917/asearch-demo/tree/main/integration)で管理します。

## ライセンス

新規コードはMIT OR Apache-2.0です。[NOTICE](NOTICE)と各crateのライセンスを参照してください。公開CI・Pagesの配信確認と、ローカルのテスト結果は区別します。

## タグリリース

Cargoのversionに対応する`v0.1.0`形式のタグをpushすると、Rustのfmt/Clippy/test/fuzzとOS別テストを通過した成果物だけをGitHub Releaseへ公開します。

- `asearch_wasm.wasm`: 基本命令版WASM。
- `asearch-cli-PLATFORM`: Linux x86_64、macOS aarch64、Windows x86_64のCLI（Windowsは.exe）。
- `asearch-assets-PLATFORM.tar.gz`: WASM・CLI・ライセンスの同梱bundle。
- `SHA256SUMS`: Release AssetsのSHA-256。

デモは公開済みbundleのタグとSHA-256を固定し、回帰テスト通過後にPagesへ反映します。コンパイル時パスはRelease CIで一般化します。runnerの構成は[GitHub公式仕様](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)に従います。実際のタグ公開・ホステッドCI・配信確認はローカル検証とは別のゲートです。

## デモへのリリース連携

Rust CI、OS別ビルド、Release公開に成功すると、`notify-demo`が公開タグを`asearch-demo`へ通知し、回帰テストとPages配信を起動します。demoへインストールしたGitHub AppにActionsの書込み権限と自動付与のMetadata権限を与えます。repo固有の`GITHUB_TOKEN`だけでは別repoを起動できません。

このrepoのActions設定へ、AppのClient IDを変数`ASEARCH_DEMO_APP_CLIENT_ID`、秘密鍵をSecret `ASEARCH_DEMO_APP_PRIVATE_KEY`として登録します。タグ公開前にdemoのActionsを有効化し、`release-update.yml`をmainへ反映してください。秘密鍵はcommitしません。AppにContentsやPagesの書込み権限は不要で、Pagesの権限はdemo自身の配信jobが持ちます。

通知だけが失敗しても、公開済みReleaseは削除しません。設定を直して失敗した`notify-demo`だけを再実行するか、demoの手動配信Actionを使います。版の選択と切り戻しは[デモの日本語配信ガイド](https://github.com/tkhshtsh0917/asearch-demo/blob/main/docs/ja/deployment.md)を参照してください。

文書ごとの内容と読む順番は[日本語ガイド](docs/ja/README.md)を参照してください。
