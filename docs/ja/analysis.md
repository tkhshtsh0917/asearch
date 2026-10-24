# ネイティブの日本語解析

[English](../analysis.md) · 日本語版

`asearch-analyzer`は、Sudachiの形態素解析、フィルタ、方向付き同義語、ローマ字読みを担当するサーバー専用crateです。依存するのは`asearch-cli`だけで、`asearch-wasm`・`asearch-engine`・`asearch-builder`は解析器や言語辞書を読み込みません。ASIX v1は変更せず、既存の`texts`へ原文と補助表現を登録します。

```sh
asearch-cli compile-analysis-resources stopwords.txt stoptags.txt synonyms.txt resources.json
asearch-cli analyze system.dic resources.json policy.json questions.jsonl analyzed.jsonl
asearch-cli build analyzed.jsonl index.asix
```

解析入力は、`{"key":1,"texts":["ビートルズの曲は？"]}`のように、keyあたり原文1つを持つJSONLです。`order`は省略可能です。出力は、原文を先頭に補助表現を加えた通常のbuilder用JSONLです。表示する原文・URL・帰属情報は別カタログにkeyで紐づけます。同義語は質問全文のコピーではなく独立した補助表現にし、異なるtext間にまたがる複数語の一致は扱いません。

リソース変換器はstopリストのコメントとCSVの不要列を除き、同義語グループと方向を保持します。フラグ空欄／`0`は起点・出力、`1`は出力だけ、`2`は除外です。小文字化したトークンの表層形・正規化形・辞書形と完全一致で照合し、1段だけ展開します。再帰展開や置換の総当たりは行いません。stoptagsは品詞の階層要素の前方一致で照合します。フィルタは生成する補助表現だけに適用し、原文は検索可能なまま残します。

設定例：

```json
{
  "splitMode": "C",
  "keepPartOfSpeech": ["名詞", "動詞", "形容詞", "形状詞"],
  "maxSynonymTexts": 256,
  "maxDocumentUnits": 131072
}
```

2つの数値上限は、`-1`でそのアプリケーション上限を解除できます。それ以外の非正値は拒否します。ASIXのtextあたり65,535 UTF-16単位、CLIのJSONL入力1行1 MiB／合計64 MiB・出力64 MiB・解析20,000件、builder／engineの資源上限は解除しません。上限超過時は補助表現を黙って捨てず、明示的に失敗します。

SudachiのRust依存はGit revision `1c21940d1ca6b045eb211ecc1cdb767326f3783f`に固定し、ローマ字変換にwana_kana 5.0.0を使います。ビートルズから`biitoruzu`と`bi-toruzu`を生成します。英語名の`Beatles`はローマ字変換では得られず、同義語の登録で扱います。辞書の収録範囲と検索品質はアプリケーション側で評価します。

CLIはSudachiに組み込まれた標準プラグインと、明示されたシステム辞書のパスを使います。Sudachiはシステム辞書をmmapで読み込みます。解析器の生存中はファイルを変更せず、ハッシュ付きの不変ファイルと参照先の原子的な切り替えで更新します。変換済みstop／同義語リソースは小規模な構造化JSONとして一度だけRustの所有するコレクションへ読み込み、追加のmmap層は設けません。

20,000件は解析1回あたりの制限で、ASIX全体の文書数上限ではありません。大きなコーパスでは解析結果をバッチごとに結合して`build`へ渡せますが、builder・クライアントのtext数・単位数・メモリ予算も満たす必要があります。デモの50k検証では574,363個の検索用text・40,632,640バイトの基本索引となり、256 MiBのクライアント作業予算で静的な読み込み・検索が成功しました。ブラウザ全体のRAM使用量や更新時の余裕を保証する値ではありません。

ネットワーク取得と上流revision・ハッシュの管理は呼び出し元の責務です。[デモの解析パイプライン](https://github.com/tkhshtsh0917/asearch-demo/blob/main/docs/ja/search-analysis.md)に、明示的な最新版取得、通常ビルドの版固定、整合性検査、不変の辞書キャッシュ、出典表示、ブラウザ回帰テストを用意します。ネイティブ依存のライセンス・帰属情報はRelease Assetsの`licenses/server-dependencies/`へ同梱します。
