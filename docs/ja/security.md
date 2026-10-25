# セキュリティと依存関係の保守

[English](../security.md)

このrepoはRustソース、ネイティブCLIバイナリ、読み取り専用の検索WASMを配布します。CargoのlockfileにBrotli依存はありません。HTTPスタック、ブラウザ・ランタイムの更新、配信設定は呼び出し元が管理し、Rustの依存監査ではそれらを検査しません。

```sh
mise run audit
```

版固定したcargo-auditが、Cargo.lockとfuzz/Cargo.lockを最新のRustSecデータベースで検査します。CIではRelease公開前にこの検査を行い、週次・手動のworkflowで新たに報告された問題も確認します。データベースやネットワークの失敗時は検査も失敗します。DependabotはCargoとGitHub Actionsの更新を提案し、マージ前にレビュー・テストします。指摘0件の監査は、その時点の既知のadvisoryについての結果です。

信頼できない入力に対して、ASIX検証、確保上限、位置計算の処理量上限、原子的採用を維持してください。Releaseのchecksumは不一致を検出しますが、信頼できない配布元の真正性を保証しません。認証情報をソースや公開成果物へ含めず、CIの変更時はworkflowの権限も確認してください。

別repoのデモはNodeとTSの依存関係を独立に管理します。CVE-2026-111497に対応したNodeの要件を含む[セキュリティガイド](https://github.com/tkhshtsh0917/asearch-demo/blob/main/docs/ja/security.md)を参照してください。

脆弱性の疑いは、有効になっている場合はGitHubの非公開脆弱性報告を使ってください。利用できない場合は、詳細を公開せず非公開の連絡手段を依頼してください。認証情報や機微な再現資料を公開issueへ記載しないでください。
