# Rust版の日本語ガイド

[English](../README.md)

[README.ja.md](../../README.ja.md)はRust本体の構成、ビルド、CLI入力、互換性、タグリリース、demoへの通知を説明します。

| 読みたいこと                           | 読む場所                                                                                                 |
| -------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| ビルドして使う                         | [日本語README](../../README.ja.md#ビルドと検証)                                                          |
| crateの責務と依存方向                  | [構成](../../README.ja.md#構成)                                                                          |
| Webサービスでのcrateの連携             | [アーキテクチャ](architecture.md)：サーバー・CDN・ブラウザの連携図と更新の責務                           |
| Bitapのビット演算を順に追う            | [アルゴリズム](algorithm.md)：GIF、状態表、式、一括検索との違い                                          |
| ASIXの何バイト目に何が入るか           | [バイト配置](asix-format.md)：ヘッダ全欄、セクション表、256バイトの実例                                  |
| 個別の型・関数の責務、制約、工夫       | `cargo doc --workspace --no-deps`で生成するAPIドキュメント                                               |
| CLIのJSONL入力とサーバー生成           | [Rustでの利用](../../README.ja.md#rustでの利用)                                                          |
| Release Assetsと配信連携               | [タグリリース](../../README.ja.md#タグリリース)、[デモへの連携](../../README.ja.md#デモへのリリース連携) |
| 検索意味論、ASIX、更新、WASM APIの詳細 | [demoの日本語仕様](https://github.com/tkhshtsh0917/asearch-demo/tree/main/integration/docs/ja)           |

Rust本体は検索と索引形式、CLI、WASM境界を管理します。このdocsでは実装に対応する構成・アルゴリズム・バイト配置を説明します。TS/JSの利用・ブラウザ検証・画面の契約はdemo repoで管理し、個別APIの責務はDoc Commentsで保守します。SVGは拡大可能で、GIFには静止画と本文による代替説明もあります。
