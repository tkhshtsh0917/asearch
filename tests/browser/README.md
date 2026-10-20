# ブラウザ統合テスト

公開API・初期化・配布物をPlaywrightのChromium/Firefox/WebKitで検証するためのハーネスです。利用者向けのUIデモではありません。Rust製品やnpm配布物からは依存しません。

| 配置                  | 責務                                                 |
| --------------------- | ---------------------------------------------------- |
| *.spec.ts             | ブラウザでの互換照合・初期化失敗と再試行・配布物検証 |
| fixtures/             | テスト専用HTMLとmodule Worker                        |
| server.mjs            | loopback限定の配信とtarballの一時展開                |
| playwright.config.mts | 実行対象・ブラウザ・server起動の設定                 |

リポジトリルートで`mise exec -- pnpm build`後、`mise exec -- pnpm test:browser`を実行してください。ブラウザ導入は[ルートREADME](../../README.md)を参照します。fixturesは製品の`dist/web.mjs`を読み、テストのために別のRust/WASMを実装しません。

UIデモを追加する場合はルートの`examples/`へ置き、公開APIを呼び出す利用者として実装します。テスト用の障害注入・fixture生成・配信用ハーネスはここへ残します。
