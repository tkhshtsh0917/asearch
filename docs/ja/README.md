# Rust版の日本語ガイド

[README.ja.md](../../README.ja.md)はRust本体の構成、ビルド、CLI入力、互換性、タグリリース、demoへの通知を説明します。

| 読みたいこと                           | 読む場所                                                                                                 |
| -------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| ビルドして使う                         | [日本語README](../../README.ja.md#ビルドと検証)                                                          |
| crateの責務と依存方向                  | [構成](../../README.ja.md#構成)                                                                          |
| 個別の型・関数の責務、制約、工夫       | `cargo doc --workspace --no-deps`で生成するAPIドキュメント                                               |
| CLIのJSONL入力とサーバー生成           | [Rustでの利用](../../README.ja.md#rustでの利用)                                                          |
| Release Assetsと配信連携               | [タグリリース](../../README.ja.md#release-assets)、[デモへの連携](../../README.ja.md#release-assets) |
| 検索意味論、ASIX、更新、WASM APIの詳細 | [demoの日本語仕様](https://github.com/tkhshtsh0917/asearch-demo/tree/main/integration/docs/ja)           |

Rust本体は検索と索引形式、CLI、WASM境界を管理します。TS/JSの利用・ブラウザ検証・画面はdemo repoの責務です。詳細仕様をこのrepoにも複製せず、実装ごとのAPI説明はDoc Commentsで保守します。
