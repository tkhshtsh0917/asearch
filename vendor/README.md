# Vendored upstream

`upstream/`は互換性の基準となる上流の原本です。製品コードと区別し、このディレクトリ全体をformatter/linterの対象外にします。

- [出典・版・ライセンスとSHA256](upstream/README.md)
- [Unicode正規化データの出典・再生成・ライセンス](unicode/README.md)
- fixture生成、型互換テスト、参照ツールはこの同梱コピーを読み、ネットワークから取り直しません。
- 上流の全ファイルをbyte-for-byteで保持します。移動によってSHA256SUMSの値は変わりません。
- `upstream/licenses`はビルド時に配布物へ同梱します。帰属・許諾はルートの[NOTICE](../NOTICE)も参照してください。

リポジトリのルートで`(cd vendor/upstream && shasum -a 256 -c SHA256SUMS)`を実行すると検証できます。
