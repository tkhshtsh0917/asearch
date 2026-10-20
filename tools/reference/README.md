# 実行可能な参照モデル

製品から独立したPython/Nodeのモデルです。意味論・アルゴリズムの検証に使い、速度比較の対象にはしません。現在のRust製品テストは`crates/asearch-rs/tests`、JS製品テストは`packages/asearch-wasm/test`です。

| ファイル                         | 責務                                         |
| -------------------------------- | -------------------------------------------- |
| ref.py                           | 上流のbit遷移、Idealの素朴DP、転置DPのモデル |
| oracle.js                        | 同梱上流の公開JSを実行                       |
| verify.py                        | レプリカ・転置DP・素朴DP・上流を差分検証     |
| characterize.py / first_level.py | 非単調levelと最初の一致levelの性質を検証     |
| verify_ext.py                    | DAG、OSA、増分行と独立oracleを比較           |
| check_lib_quirks.js              | 上流の既知の境界挙動を確認                   |
| src_oracle.cjs                   | TypeScript 5.9.3で上流ソース意味論を実行     |

## 実行方法

Python 3.10以上とNodeが必要です。リポジトリルートで実行してください。`-I -B`はPythonの環境依存とcache出力を抑えます。

```sh
mise exec -- node tools/reference/check_lib_quirks.js vendor/upstream/asearch.ts/lib/index.js
mise exec -- python3 -I -B tools/reference/verify.py vendor/upstream 31 3000
python3 -I -B tools/reference/characterize.py 7
python3 -I -B tools/reference/first_level.py
python3 -I -B tools/reference/verify_ext.py
mise exec -- node tools/reference/src_oracle.cjs vendor/upstream
```

ソースoracleは`tools/fixtures`へ隔離したTypeScript 5.9.3を使います。上流のコード・出典・ハッシュは[vendor](../../vendor/README.md)、規範は[仕様索引](../../docs/spec/README.md)を参照してください。

## 検証の限界

DAGや転置DPのモデルが通っても、製品にその機能が実装済みになるわけではありません。バイナリ形式、資源上限、更新とセッションの整合性は製品テストでも検証します。廃止したクライアントoverlay案の検証コードは現行モデルへ含めません。

JSとPythonの大小文字変換の一致を全Unicodeに一般化しません。locale依存はJSで生成したmaskと製品fixtureで別途確認します。
