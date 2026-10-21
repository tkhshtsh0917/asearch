# 索引形式とサーバー更新

[仕様索引](README.md)へ戻る。節番号は分割前から維持する。

M0以後の計画を含む。実装済みの範囲は[開発計画](../PLAN.md)を参照する。

<a id="section-11"></a>

## 11. 索引バイナリ形式（v1）と CDN 配信

Rustの単一base APIは`AsearchIndex::to_bytes/from_bytes`で実装済み。writerはTEXT/TLEN/KEYS/ORDRと、原文があればOLEN/OTXTを出力する。loaderはセクションを範囲・tagで検証し、長さ/配置順/descriptor/XXH3/原文正規化との一致を調べてから採用する。未知の非criticalセクションは検証後に読み飛ばし、再保存では引き継がない。from_bytesの入力slice自体は呼出側所有で、内部decode/stagingと完成索引の容量を予算へ含める。原文のないファイルから原文を捏造しない。Rustのfrom_deltaは昇順・重複なしTOMBを持つ不変segmentを生成し、loader/writerはTOMBを検証・保持する。TOMBは同じsegmentのtextを無効にしない。複数segmentへはIndexSetのlive maskとatomic更新で適用する。fromStreamと所有輸送バッファの容量合算、およびnative/WASMの同一文書からのバイト一致も検証した。expected descriptorのbytes/texts/id/SHA-384検証はRustのExpectedSegmentとJS LoadOptions.expectedで実装し、Node cryptoおよびWeb Cryptoとの一致を検査する。複数segment採用を含むM4全体の受入とは分ける。

<a id="section-11-1"></a>

### 11.1 方針

- 1 ファイル = 1 セグメント（§20）。サーバーで構築して CDN に置き、受け取ったバイト列をそのまま WASM メモリにコピーして使う。
- **照合のために保存するのは、正規化済みの原文（UTF-16 code unit）、text 長、key、（任意で）order と tombstone だけ**（表示と `explain` 用に、正規化前の原文 OLEN／OTXT を任意で足せる）。記号表、記号 ID、ブロック配置、SEP/TXT/END、EQ コンテナはロード時に WASM で作る。設計上の理由:
  1. 派生構造の再配信を避け、変更を文書単位に閉じ込める。
  2. 記号IDの再割り当てや後続の絶対offset変更を保存形式から除く。
  3. ブロック幅などの実行時パラメータを再ビルドなしに変更できる。
  4. 派生構造を別途検証する代わりに、検証済み原文から構築する。

構築費用と配信サイズの比較はM2/M4で製品を計測して評価する。

- 形式自体は圧縮しない。転送時の圧縮は HTTP の Content-Encoding（br／gzip／zstd、辞書圧縮の dcb／dcz）に任せる。ロード後のバイト列をそのまま使うため、また辞書圧縮を効かせるため。

<a id="section-11-2"></a>

### 11.2 レイアウト

```text
ファイル = Header（64 B）| セクション表（16 B × section_count）| セクション（各 16 B 境界、隙間は 0 埋め）
Header（little-endian）:
   0  [4]  magic "ASIX"
   4  u16  major = 1
   6  u16  minor = 0
   8  u32  header_len = 64 + 16 × section_count
  12  u32  section_count
  16  u64  file_len               ファイル全体のバイト数（切り詰めの検出）
  24  u64  file_xxh3              ファイル全体の XXH3-64（seed 0）。計算時はこの 8 バイトを 0 とみなす（ヘッダとセクション表も守る）
  32  u8   symbol_unit            0 = code point（AsearchTs／Ideal）、1 = UTF-16 code unit（AsearchTsLib）
  33  u8   reserved = 0
  34  u16  flags = 0（予約）
  36  u32  n_texts
  40  u32  n_units                TEXT の code unit 数
  44  u32  n_tombstones
  48  u16  normalizer_id          0 = none、1 = ja-basic
  50  u16  normalizer_version     1
  52  u32  normalizer_options     bit i = 項目 i+1 が有効（ja-basic-v1 の一括 API 既定は 0b11111 = 31）
  56  u64  reserved = 0
セクション表の項目: u32 tag（ASCII 4 文字）| u32 flags（bit0 = critical）| u32 offset | u32 len
```

| tag            | 種別              | 内容                                                                                                                                                                                                                                 |
| -------------- | ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `TEXT`         | critical          | u16[n_units]。正規化済みの UTF-16 code unit を配置順（§11.3）に連結                                                                                                                                                                  |
| `TLEN`         | critical          | u16[n_texts]。各 text の code unit 数（絶対オフセットではなく長さ。§11.1 の 2）                                                                                                                                                      |
| `KEYS`         | critical          | u32[n_texts]。各 text の key                                                                                                                                                                                                         |
| `ORDR`         | critical・任意    | u32[n_texts]。各 text の order。無ければ order = key                                                                                                                                                                                 |
| `TOMB`         | critical・任意    | u32[n_tombstones]。昇順・重複なしの key。このセグメントより古いセグメントにある、その key の text をすべて無効にする（§20.2）                                                                                                        |
| `OLEN`／`OTXT` | 非 critical・任意 | 正規化前の原文（u16[n_texts] の長さと UTF-16 の連結）。`explain` の原文オフセット用。無い索引では、呼び出し側が原文を渡したときだけ原文オフセットを返し、渡さなければ正規化後の text のオフセットを返す（coordinateSpaceを明示、§9） |

- 未知の critical セクションがあればロードを拒否し、未知の非 critical セクションは読み飛ばす（PNG のチャンク規則と同じ考え方）。minor を上げる変更は非 critical セクションの追加だけ。critical セクションの追加や意味の変更は major を上げる。
- オフセットは u32 なので 1 ファイルは 4 GiB 未満。text 長は 65,535 code unit 以下（§0）。
- 旧案（記号表・記号 ID・オフセット・EQ を保存し、block_words をヘッダに持つ）は §11.1 の理由で廃止した（§16）。

<a id="section-11-3"></a>

### 11.3 配置順と決定性

- text は `(order, key, key 内の text の並び)` の昇順に置く。抽出（§4.4）はスロット順＝この順なので、各 level 内の並びがそのまま順位になる。
- **決定性（MUST）**: 出力バイト列は、（文書の集合、オプション、形式の版、正規化器の版）だけで決まる。入力の並び順、ビルド環境（ネイティブか WASM か、OS）、時刻に依存させない。タイムスタンプを入れない。ハッシュ表の走査順を出力に漏らさない。同じ key の重複入力はエラーにする。OLEN／OTXT は、入力に原文があれば保存し、compaction と `toBytes()` は既存の OLEN／OTXT を引き継ぐ（正規化済みの text からは復元できないため）。
- 配信時はファイルの SHA-256 をファイル名に入れる（例: `base.<SHA-256 の先頭 16 桁>.asix`）。SRI 用の `sha384-…` もビルダが計算してマニフェストに書く（手で書かず、ビルダの出力から生成する）。

<a id="section-11-4"></a>

### 11.4 ロードと検証

- 検証（MUST。O(ファイル長)、panic 禁止、cargo-fuzz の対象）: magic、対応する major、`file_len` と実際の長さの一致、セクションが範囲内・16 B 境界・重複なし、XXH3、`Σ TLEN = n_units`、各セクション長と n_texts の整合、配置順（§11.3）、TOMB の昇順、normalizer の (id, version, options) と symbol_unit が実行時の設定に対応していること。
- XXH3 の検査時間は未計測（M2 のロード時間ベンチに含める）。
- 資源の上限は §11.6。`file_len` の上限（既定 256 MiB）は入力バイト数の制限であり、派生構造を含むメモリ上限の代用にはしない。
- JSのバイト入力はTypedArrayの組み込みgetterで実際のbuffer・offset・lengthを取得し、外部配列の独自getter・メソッド・speciesを呼ばない。`fromBytes`は実長に対する輸送予算検査後に固定長のコピーを作り、wasm-bindgenへ渡す。ストリームおよび更新のchunkは固定長の通常の`Uint8Array`ビューへ変換し、残容量の検査とコピーで同じ範囲を使う。ストリーム全体の追加コピーは作らない。
- 派生構造の構築: 記号の計数（BMP は 65,536 要素の表、非 BMP は小さな表）→ 密な記号 ID → ブロック配置 → SEP/TXT/END と rank の累積 → EQ コンテナ（§3.3）。観測（代理実装。上の手順のうち記号の計数・ID・配置・マスク・CSR だけ）: 105 万 code unit で 3.4〜3.7 ms、1,050 万で 65〜70 ms（§15.2 (d)）。
- ストリーミング: JS 側は最大 64 B のヘッダ用バッファで開始し、`file_len` の安全な変換・magic・版・§11.6 の上限を検査してから入力バッファを一度だけ確保し、以降の chunk をその位置へ `set` でコピーする。各 chunk のコピー前に残り容量を検査し、超過、EOF の不足、失敗時は reader を cancel/release して candidate を破棄する。WASM メモリの grow が起きうる await の後は TypedArray を取り直し、古いビューへ書かない。`await response.arrayBuffer()` のように全体の一時コピーを作らない（一時的にファイル 1 つ分のメモリが余分に要る）。
  - BYOB リーダーで WASM メモリへ直接読むことはできない（観測: Node v22.22.0 で `TypeError: ArrayBuffer is not detachable and could not be cloned`。WASM メモリの ArrayBuffer は移譲できないため。ブラウザでは未計測）。chunk ごとに 1 回のコピーが下限。
  - `Content-Length` は Content-Encoding が付くと圧縮後の長さなので、確保量に使わない。
- WASM のコンパイル（`compileStreaming`）と索引の取得は並行に始める（例は README に置く。取得そのものは範囲外）。重い構築を避けたい場合は Worker でロードし、検索も同じ Worker で行う。

<a id="section-11-5"></a>

### 11.5 版の互換

- 正規化器の (id, version, options) の挙動は、公開後に変えない（MUST）。挙動を変えるときは新しい version にし、実行時は公開済みのすべての version を読めるようにする（CDN やブラウザのキャッシュに残った古い索引を、新しい WASM でも読めるようにするため）。
- 正規化器は冪等（N(N(x)) = N(x)）であること（MUST、テストする）。compaction（§20.5）が、正規化済みの text から作り直しても元の文書から作った結果と一致するため。
- 実行時は同じ major のすべての minor を読める。major を上げた場合、ビルダは少なくとも 1 つ前の major でも出力できるようにする（SHOULD）。

---

<a id="section-11-6"></a>

### 11.6 資源予算と失敗時の不変条件（拡張経路はM1、索引固有項目はM2以降）

以下の数値は **設計上の暫定既定値**であり実測による安全保証ではない。M2/M4 のピーク計測で校正し、変更時は D12 と予算定数を更新する。`ResourceLimits` は構築・ロード時に指定し、各 instance の全セグメント・全sessionの合計に適用する。

| 項目                     | 暫定既定値 | 対象                                                                           |
| ------------------------ | ---------: | ------------------------------------------------------------------------------ |
| `maxBytes`               |    256 MiB | ロードする 1 ファイルの非圧縮バイト数                                          |
| `limits.maxTotalBytes`   |    256 MiB | 保持する入力と staging 入力の合計。置換時は旧＋新を含む                        |
| `limits.maxTexts`        |  1,000,000 | 空 text・死んだ text・staging を含む保持 text 数                               |
| `limits.maxUnits`        | 10,000,000 | 保持する正規化済み text の UTF-16 code unit 数                                 |
| `limits.maxSlots`        | 16,777,216 | 境界・padding・guard を含む全派生レイアウトのスロット数                        |
| `limits.maxWorkingBytes` |    512 MiB | コア所有の入力・派生配列・Workspace・全session cache・staging の確保容量の合計 |
| `limits.maxPatternUnits` |     65,535 | 1 回の検索入力と正規化後入力の各 UTF-16 長。31 記号の語幅制限とは別            |
| `limits.maxSegments`     |          8 | サーバーセグメント数（1..8で設定）                                             |

ResourceLimitsは論理index単位で、子sessionと更新stagingを同じ台帳へ含める。独立した別index、JS所有の結果コピー、アプリ全体のheap量を合算する設定ではない。WASM memoryのページ容量は解放後も縮まないため、live予約量と区別する。

- 全値を正の安全な整数として検査し、和・積・整列・u64→usize/u32/JS number の変換は checked arithmetic で行う。未知・重複・重なったセクション、予約値違反、異常な `header_len/section_count` も検査し、ヘッダ用確保を先に増やさない。TOMB 件数、原文 OTXT、記号表等も maxBytes と maxWorkingBytes の対象にする。
- 段階: ヘッダとファイル長 → セクション境界/個数 → TLEN 等から個数/スロットを見積もる → 派生構造を構築。**各大きな確保の前**に残予算と計算結果を検査する。空 text 数は n_units の上限では抑えられないため、maxTexts も独立に適用する。
- 構築と更新の確保は fallible にし、予算超過・予約失敗を `RESOURCE_LIMIT` として返す。内部のプログラム誤り、ホスト全体の OOM や外部からの強制終了を回復可能と偽らない。maxWorkingBytes はコアの容量予算であり、JS heap・JIT・ブラウザプロセス全体の上限ではない。WASM linear memory の高水位とピークは別指標として計測する。
- セッションは登録/解放時に予算を予約/返却する。32 MiB の行キャッシュ既定（§4.7）も総予算の内側。足りなければ cache を捨てて全量計算する。結果バッファや全量計算に必要な Workspace すら確保できないときは失敗する。上限を外して継続しない。
- 更新/ロードは candidate で検証して一度に採用する。失敗時は旧実効文書集合、更新世代、成功済みセッションの論理状態を維持する（cache の破棄と借用ビューの失効は可）。old+new が予算に入らない置換は旧索引を先に捨てず失敗させ、別 Worker/instance でのロードを呼出側が選べるようにする。
- WASM境界では、初期入力とflat配列の合計をコピー前に検査し、構築中も輸送容量を予算から差し引く。検索・説明・更新の入力も既存indexの残予算で越境前に検査する。固定メタデータ分には保守的な余裕を残す。
- `fromStrings/fromDocs/buildIndex` にも同じ予算を適用し、Iterable を無制限に collect しない。入力は有限とする。IntentIndex のパース長・深さ・DAG ノード数/辺数は M6 着手前に追加予算を定める。展開数が少なくても無限の構築資源は認めない。

---

<a id="section-20"></a>

## 20. 配信と更新（セグメント、D8・D9）

<a id="section-20-1"></a>

### 20.1 更新頻度ごとの方式（目安）

- 更新が少なければ新しいbaseを生成して全量置換する。
- 更新が多ければ追加・変更・削除をdelta＋tombstoneとして生成し、定期的にサーバーでcompactionする。頻度と閾値はM4で校正する。
- 運営者の保存 → 呼出側のサーバー処理が索引/差分を生成・公開 → 各ユーザーのアプリが取得 → asearchが検証・採用、という責務分担。全ユーザーへの同時・即時反映は保証しない。検知・ジョブ起動・CDNへの配置・manifest取得/通知/ポーリング・画面描画は呼出側の仕事で、asearchはそれらを実装しない。
- 既存索引をブラウザ内だけで編集するAPIは持たない。初期構築のfromStrings/fromDocs/buildIndexは残し、既存索引の文書単位編集と区別する。

<a id="section-20-2"></a>

### 20.2 データモデル

- 文書 = (key: u32, order: u32, texts: 1個以上)。keyは版をまたいで安定なID、orderは小さいほど上位（既定key）。言い換え等は同じkeyの別textとする。
- 索引はサーバー由来の不変セグメント列（古い順）。実効文書集合はbaseへdeltaのTOMBと新textを順番に適用したもの。結果はkey単位の最良textで、(level, order, key)順に並ぶ。
- セグメントsのTOMBはsより古いセグメントの該当key全textを無効化し、s自身のtextは殺さない。変更は同一delta内のTOMB＋新text、削除はTOMBのみ、追加は新textとして表す。順位変更も文書全体の変更として扱う。
- 削除印は `dead` の1種類。`LIVE_END = END & ~dead`。ブラウザ内での優先順位層やforgetによる復元は持たない。
- 実効集合で1つのkeyが生きているセグメントは高々1つ。ビルダはこれを保証し、クライアントは検証して違反を `INVALID_INDEX` として採用前に拒否する。同じkeyの複数textは同一セグメント内で許す。

<a id="section-20-3"></a>

### 20.3 照合とマージ

- 各セグメントは独立したレイアウト/記号表/EQを持ち、クエリのコンパイルもセグメントごとに行う。
- TOMBは各セグメントのLIVE_ENDに適用し、行計算は変えない。keyから旧text群を引く表の構築・保持は予算に含める。1textのdeadビット変更は定数時間でも、1keyの全text特定/削除全体をO(1)と主張しない。
- セグメントのカーソルを(level, order, key)でk-wayマージし、limit件の異なるkeyが揃えば打ち切る。countTotalは§4.4に従って最後まで計数する。死textが増えたときの計算の無駄はサーバーcompactionで回収する。
- 更新の新契約は§14.1のU4/M4で検証する。

Rustの`IndexSet::new(Vec<AsearchIndex>, limits)`は完成済みsegmentの所有権を取り、全保持text/units/slots/入力容量とworkspaceを合算する。TOMBを古い文書へ適用してからlive maskを作り、同じkeyが複数segmentで生きていれば拒否する。searchはlive文書だけのsegment cursorをk-way mergeし、厳密totalsをlimitと独立に数える。to_bytesはlive行を直接保存して、別のengineを作らずにcanonical baseへ圧縮する。Rustのappend_bytes/replace_bytesはold+stagingの各資源を検査してcandidateを構築し、採用後だけrevisionを進める。現行のネイティブ更新はworkspaceの解放を要求し、残っていればBUSYを返す。queryを外部へ保持したままworkspaceを再作成できる。reserve_workingの外部容量予約は更新後も同じ台帳へ残り、段階的構築時の一時上限は採用後のruntimeへ持ち越さない。WASMの非同期add/replaceは旧索引の検索を継続しながら固定輸送バッファを共有台帳へ予約し、candidateと参照表が準備できてから一括採用する。子sessionのworkspaceを更新開始時と採用直前に解放し、入力/オプションを保って次回検索で再準備する。失敗時は旧列とrefsを保持する。

v1のOLEN/OTXTはsegment全体の原文保持を表すため、同じ実効集合に原文あり/なしの文書が混在するcolumnは採用前に拒否する（D25）。全生存文書が同じ保持方式なら、死んだ文書や削除だけのdeltaの方式は影響しない。原文を捏造したり、既存の原文を捨てて圧縮したりしない。

<a id="section-20-4"></a>

### 20.4 セグメント採用とセッションの整合性

- addSegmentは新セグメントと、旧セグメントへ適用するTOMBをcandidate内で検証してから一括採用する。旧LIVE_ENDを検証途中に直接書き換えない。途中失敗なら旧文書集合/世代を保持する。
- 新セグメントは現在の入力で新規に評価する。旧セグメントの行cacheは、レイアウトが同じなら再利用可。LIVE_ENDが変わったセグメントの抽出結果・totals・集約結果は無効化する。
- replaceSegmentsは新サーバー列全体からdead印を再計算する。manifestからTOMBセグメントが外れると、再利用セグメントでも生存集合が増える場合がある。このときNarrowing候補を無効化し、全量計算へ戻す。新baseなら旧layoutのcacheは再利用しない。
- 全更新成功でrefsは失効する（§12.4）。失敗時は既存セッションの論理状態を維持し、成功後も保持した入力/オプションに対する全量検索と同じ結果を返す。新しいtextが前回の候補集合やtop-Nに無かったことによる取りこぼしを許さない。
- 資源不足・二重更新・free時の規則は§11.6/§12.4。クライアント内の可変セグメントやそのcompactionは作らない。

<a id="section-20-5"></a>

### 20.5 マニフェストと compaction（サーバー側の構築）

マニフェスト（JSON、短い TTL で配信。値はすべてビルダが出力し、手で書かない。下は `<…>` を値に置き換える雛形）:

```text
{
  "format": "asearch-manifest/1",
  "indexId": "example-faq",
  "generation": 42,
  "index": { "major": 1, "minor": 0, "symbol_unit": "code-point",
             "normalizer": { "id": "ja-basic", "version": 1, "options": 31 } },
  "segments": [
    { "id": "<file_xxh3 の 16 進>", "url": "base.<sha256 先頭 16 桁>.asix", "bytes": <整数>, "texts": <整数>, "integrity": "sha384-<ビルダが計算>" },
    { "id": "<file_xxh3 の 16 進>", "url": "delta.<sha256 先頭 16 桁>.asix", "bytes": <整数>, "texts": <整数>, "integrity": "sha384-<ビルダが計算>" }
  ]
}
```

- `indexId`は索引系列の識別子（ASCII `[A-Za-z0-9][A-Za-z0-9._-]{0,127}`）。CLIのmanifest出力で明示指定し、サーバー再起動/compactionで変えない。同じ系列の `index`（major/minor、symbol_unit、normalizerのid/version/options）は不変。設定を変更する再構築は新しいindexIdを発行し、呼出側で新しいindexとしてロードする。既存セッションを暗黙に別の正規化体系へ移さない。
- `generation`は1以上の安全な整数で、同一系列内で単調増加。同じgenerationでは全ての規定フィールドが同じでなければならない。JSONオブジェクトのキー順は比較に影響させず、セグメント配列順は意味を持つ。上限到達時は折り返さず、新系列への明示移行を要求する。
- `diffManifest(current: Manifest | undefined, manifest)` は取得も状態変更もしない純関数。currentは **適用完了した完全なmanifest**。SegmentInfoだけでは設定/descriptor/系列を検査できないため代用しない。manifestはサーバー由来のセグメントだけを記述する。
- 判定順: ①両manifestの形式/型/安全な整数、index定義、重複segment ID、descriptorを検査（不正は `INVALID_MANIFEST`） ②currentなしならreplace ③indexIdまたはindex定義が異なれば `CONFLICT`（自動移行しない） ④古いgenerationはstale ⑤同一generationの内容差は `CONFLICT`、同一内容はnone ⑥新しいgenerationでは共有IDのbytes/texts/integrityの変更を `CONFLICT` とし、ID列がcurrentの完全な接頭辞ならappend（同長ならnone）、それ以外はreplace。URLだけの移転は新しいgenerationなら許す。staleのdescriptorで現在の既知データを上書きしない。
- manifestのsegmentsはbaseを含む1..limits.maxSegments個（空コーパスも0textのbaseを持つ）。diffManifest単体は1..8個をv1の運用上限として検査する。
- descriptorのidはXXH3-64の16桁小文字hex、bytes/textsは範囲内の整数、integrityは正しい長さのSHA-384 SRI、URLは空でないHTTP(S)の絶対/相対参照（userinfo禁止）として検査する。URLの解決とorigin/CORSポリシー、ネットワーク取得は呼出側。XXH3は誤破損検査であり暗号学的な真正性を保証しない。
- loaderのexpectedを渡した場合、実データの長さ・text数・file_xxh3とdescriptorを照合し、ファイル全体のSHA-384を計算してintegrityを検証してから採用する。WASM/native双方で同じ検査を行い、hash検査も資源/ロード時間の測定に含める。trusted manifestと通信経路の信頼は呼出側の責任で、manifestそのものが改ざんされる場合の認証は提供しない。
- 呼出側は同じ論理索引のdiff→load→採用→current保存を直列化し、append/replaceの全candidateの検証と一括採用が成功してからcurrentを更新する。複数segmentのmanifest適用はreplaceSegmentsへ完全な新サーバー列を渡して一括採用する（既知segmentの再利用は可）。addSegmentの連続呼出しは複数segmentのatomic適用を保証しないため、この用途には使わない。noneでも新generationの完全manifestを保存する。失敗時は旧currentを維持し、同じmanifestで再試行できる。
- CLI: `asearch-cli build`（全量）、`asearch-cli delta`（変更リスト → delta セグメント）、`asearch-cli manifest`（検証済みのサーバー列 → 次世代マニフェスト）、`asearch-cli compact`（生きている文書から新しい base）。入力は JSONL（文書は `{"key":…, "order":…, "texts":[…]}`、変更は `{"op":"upsert", …}` か `{"op":"remove", "key":…}`。これはビルダ入力の操作名で、ブラウザの編集APIではない）。同じビルダを WASM 版として npm の `buildIndex` からも使える（Node やエッジのサーバーで Rust ツールチェーン無しに構築するため）。**ネイティブ版と WASM 版の出力はバイト単位で一致すること（MUST、テスト）。**
- compaction の既定（暫定。セグメント数と 1 打鍵の時間の関係をベンチで測って校正し、定数ファイルに記録する）: セグメント数が 4 を超えた、delta の text 数の合計が base の 10% を超えた、または base の死んだ text が 20% を超えたら、新しい base を作る。
- compaction の結果は、同じ文書集合から `build` した結果とバイト単位で一致すること（MUST、テスト。§11.5 の冪等性が前提）。

<a id="section-20-6"></a>

### 20.6 配信の推奨手順（非規範。CDN の設定そのものは範囲外）

- セグメントは内容ハッシュ付きの URL にして `Cache-Control: public, max-age=31536000, immutable`。マニフェストは短い max-age か `no-cache`（ETag で再検証）。
- 転送圧縮はCDN/HTTPサーバー側で設定する。ASIXのContent-Typeを圧縮対象へ含め、Content-Encodingとキャッシュの設定を環境ごとに検証する。このCLIはASIX bytesを出力し、事前圧縮・辞書圧縮・CDN設定を提供しない。loaderはブラウザで復号されたbodyとASIXの宣言長を検証する。

<a id="section-20-7"></a>

### 20.7 メモリ

- WASM のメモリは縮まない。全量を置き換えると新旧が同時に載るので、ピークは索引の約 2 倍になる。delta で済ませれば増分だけで済む。
- 長時間開くページで全量の置き換えを繰り返すなら、世代ごとに Worker（＝別の WASM インスタンスとメモリ）を作り、古い Worker を終了してメモリを解放する手がある。wasm-bindgen の生成コードが同じスレッドで複数のインスタンスを扱えるかは未検証なので、Worker ごとに分ける方を推奨する。

<a id="section-20-8"></a>

### 20.8 v1 の範囲外

- ブラウザ内だけの編集/上書き層（overlay）、upsert/remove/forget/forgetAll等のクライアント編集API。
- IntentIndex（§7）の差分更新（全量再構築だけ）。
- 更新（tombstone）と Worker 分割の併用（§12.3 の注）。
- マニフェストの取得、ポーリング、プッシュ（WebSocket／SSE）、オフライン保存（Cache API／IndexedDB）、CDN の設定。
