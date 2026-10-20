# テストと性能検証

[仕様索引](README.md)へ戻る。節番号は分割前から維持する。

M0以後の計画を含む。実装済みの範囲は[開発計画](../PLAN.md)を参照する。

<a id="section-14"></a>

## 14. テスト計画

1. **フィクスチャ**: `tools/fixtures/gen.mjs` が `vendor/upstream/` の同梱コピー（asearch.ts@1.0.3 の公開物と TS ソース、asearch@1.0.1。版と integrity は `vendor/upstream/README.md`）を実行し、ランダム＋敵対ケースの期待値を生成する。敵対ケースは既知の弱点を反転して設計する: 空 pattern／空 text、空白のみ、先頭／末尾／連続空白、31／32 UTF-16 長の境界（サロゲート対を含む）、孤立サロゲート、非 BMP、大文字小文字（ß、İ、Σ）、ambig 0..5 と 31／32／33、非整数／負、Include で pattern が text より長い、短い text での非単調（Q3）。
2. **Rust プロパティテスト**（proptest）: (a) 全エンジン == ScalarWm、(b) Ideal == 素朴 DP、(c) 特性定理と系、(d) TransposedBlocked のブロック境界（text がブロック端に接する、死スロット、長大 text）、(e) DAG == 展開の総当たり、(f) OSA == 素朴 OSA DP、(g) Narrowing の結果 == 全量計算（条件成立時だけ適用されること）。
3. **確保ゼロ**: 計数グローバルアロケータで、ウォームアップ後の `search`／`SearchSession.update`／`match` の確保回数が 0 であることを検査する（ネイティブ）。索引の更新（`addSegment`、`replaceSegments`）と、その直後の初回検索は対象外。
4. **索引**（§11）: 往復（toBytes → fromBytes）で同一。決定性（入力順をシャッフルしても、ネイティブと WASM のビルダでも、バイト単位で一致）。破損・切り詰め・巨大な `file_len`・未知の critical セクション・順序違反で `Err`（cargo-fuzz、panic 0）。未知の非 critical セクションは読み飛ばす。正規化器の冪等性。
5. **セグメント**（§20）: 各textのlevelの独立性、追記で旧スロットの行が不変、サーバーのbase＋delta＋tombstone＋マージ == 実効文書集合への素朴DPをproptestで検査する。tombstone無視・key重複排除なし・level順違反などをわざと入れて検出力を確かめる。compaction == 同じ文書集合のbuild（バイト一致）、セッション付きの追加・全量置換・失敗後再試行も検査する。クライアントoverlayは受入対象に含めない。

6. **WASM**: 同じフィクスチャを Node 上で JS API 経由で照合する。Playwright で Chromium／Firefox／WebKit のスモークテストを行う（ストリーミングロード、Worker 内での実行、DOM の無い環境での実行を含む）。
7. **参照実装**: `tools/reference/` の Python／Node オラクルを CI で動かし、Rust の結果と突き合わせても良い。

---

<a id="section-14-1"></a>

### 14.1 段階別の受入テスト対応表

IDは受入項目の識別子で、実在するテストファイル名ではない。実装時に具体的なテスト名へ対応付け、マイルストーン報告に成否・件数・実行コマンドを記録する。複数のIDを1テストで済ませる場合も、どの条件を検査したか明記する。

| ID  | 導入段階 | 必須の検査                                                                                                                                               |
| --- | -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| C0  | M0       | lib/srcの全fixture、上流36assertion、README例、境界の意図的逸脱、型互換、native ambig≤31の確保ゼロ                                                       |
| I0  | M0       | §19.5.1全状態遷移、並行init、old ready、未処理rejectionなし、Node ESM/CJS、Chromiumとmodule Worker、pack済みtarballの独立利用                            |
| P1  | M1       | 同じ結果/意味論でAB/BA計測、ノイズのある架空標本では未確定、合成10%超回帰で判定、集計の再現性                                                            |
| Q2  | M2       | 3/4・7/8・11/12のK境界、wrap非計数、正規化/空白/非BMP、limit=0、空入力、Invalid option、key単位top-N/最小level別totals、削除後計数                       |
| R2  | M2       | 小さいmaxBytes/maxTexts/maxSlots/maxWorkingBytesで確保前拒否、空text多数、長大text/可変長block、OTXT/TOMB、overflow、過剰chunk/途中EOF、失敗時reader解放 |
| F2  | M2       | 決定性、native/WASM一致、unknown critical拒否/noncritical許容、予約値・重複section・checksum・配置不正、format fuzzでpanicなし                           |
| S3  | M3       | 正規化後LCP、ｶ→ｶﾞ、Backspace、K増加、mode変更、top-N外の候補の復活、cache eviction/予算枯渇時fallback、全量と一致                                        |
| U4  | M4       | read中searchは旧索引、二重更新BUSY、free中止、失敗時の旧索引/世代保持、差分追加とbase置換のキャッシュ整合、セグメント/予算上限拒否                       |
| M4  | M4       | 新系列/設定差/同世代改変拒否、古い世代stale、descriptor矛盾/SRI不一致拒否、noneでも世代更新、複数append失敗→再試行、原子的採用後だけcurrent更新          |
| E5  | M5       | 正規化前UTF-16位置、半角濁点/全角/かな、同点整列、copy後の参照値と更新世代、別index/古いref拒否                                                          |
| D6  | M6       | DAG==小規模全展開、指数的展開式の非展開、parser深さ/ノード予算、文法エラー、単語の順と最良展開                                                           |
| O7  | M7       | 任意engineの採否を記録。採用時は同値性、SIMD/basic双方、対応機能検出とfallback、安全境界、§15.1.1を満たす                                                |

- R2/U4のfault injectionは、小さな予算・失敗するallocator・streamを使い、実機のOOMを誘発しない。破損した1ケースでprocess全体が落ちないことも独立processで検査する。fuzzはseed corpusをcommitし、試行時間・実行数・対象を報告する。有限のfuzzであらゆる入力の安全性を証明したとは書かない。
- §14の基本propertyに加え、Q2/U4/M4には独立した素朴な文書集合oracleを用いる。新しく定めた予算・manifest・初期化の検査は既存のPython検証済み件数に加算しない。

---

<a id="section-15"></a>

## 15. ベンチマーク計画と観測値

<a id="section-15-1"></a>

### 15.1 計画

- データ: (C1) 合成 Zipf コーパス（s = 1.05、3,000 記号、長さ 10〜60、シード固定。生成器をM1で実装し版管理する）を 10^4〜10^7 記号、(C2) 利用者が提供する実 FAQ／展開文（リポジトリに含めない）、(C3) 敵対ケース（極短、極長、絵文字、ワイルドカード多用）。
- クエリ: コーパスの部分列にタイポ（置換、挿入、削除、転置）を注入した打鍵列。IME 的な末尾置換を含む。
- 指標: 1 打鍵のレイテンシ p50／p95／p99、m ごとの全量時間、索引の構築・ロード時間（取得開始から最初の検索可能まで）、メモリ（`memory.buffer.byteLength`）、WASM サイズ、セグメント数（1〜8）ごとの 1 打鍵時間、更新（`addSegment`／`replaceSegments`）の反映時間、配信サイズ（brotli-11 と、旧版を辞書にした差分）。
- 環境: Node（V8）と Playwright の Chromium／Firefox／WebKit。版とハードウェアを記録する。ウォームアップの後 30 回以上計測し、中央値と p95 を出す。タイマー分解能が粗いことに備え、短い操作はまとめて計測する。
- 基準: asearch.ts@1.0.3（JS）、node-asearch@1.0.1、ScalarWm（素直な移植）。結果が一致しない比較は、性能比較として無効。

<a id="section-15-1-1"></a>

### 15.1.1 公平性と回帰判定（M1 から）

- 比較単位を分離する: (A) 互換APIの1回match、(B) 全文書の最小level＋同一top-N、(C) 逐次入力セッション、(D) 構築/ロード/更新。1回の真偽判定と、K段の全件ランキングを同じ速度として割らない。ベースラインと候補で同じprofile・normalizer・mode/wrap・K・limit・countTotalと結果を照合してから測る。意味論を揃えられない比較は参考値にとどめ、速度比を公表しない。
- baseline/candidateを同じホスト・同じブラウザ版・同じseed/要求列でAB/BA交互に測る。独立した5回以上のpaired runを1組とし、両者のcache/ウォームアップ状態を同じにする。逐次列は決めた地点でresetし、前runのcacheを流用しない。他のビルド・テスト・計測を並行しない。
- 各runはウォームアップ後30標本以上。打鍵p95を主張する場合は1構成につき100打鍵以上、p99なら1,000打鍵以上の個別測定を集め、標本数を併記する（十分な統計精度の保証ではない）。短い操作をまとめた場合はバッチ平均の分布として報告し、それを個別打鍵のp95/p99と呼ばない。
- 10%は回帰候補の閾値として維持する。各paired runのcandidate/baseline中央値比から幾何平均比を求め、run単位のpaired bootstrap（固定seed、10,000再標本）の95%区間を出す。**下限が1.10を超えたときだけ回帰候補**とし、追加の独立5paired runを実行する。両組で条件成立なら回帰確定。片方だけ/区間が閾値を跨ぐ場合は未確定と報告し、都合のよいrunを選ばない。
- 通常の共有CIは正しさ・型・format/lint・ビルドを必須にし、速度の数値閾値でPRを落とさない。性能回帰確定は条件を固定した計測のリリース判定に用いる。専用runnerを導入した場合だけ同じ規則を自動ゲート化できる。
- raw標本・seed・コーパス/要求列hash・両commit・依存/ブラウザ/コンパイラー版・OS/CPU・タイマー方式・集計スクリプトを保存する。Planner閾値の変更はこの証拠と一緒にレビューする。WASM linear memory、コア容量予算、JS/プロセスメモリを区別し、未取得の指標は未取得と書く。

<a id="section-15-2"></a>

### 15.2 設計上の仮説と検証の方針

過去の代理プロトタイプの計測値は製品の性能保証に使わない。M1で同一結果・同一負荷のRust/WASM製品を計測して判断する。

- 密な候補には増分行キャッシュ、疎な候補には安全条件付きNarrowingを検討する。
- 行計算だけでなく抽出・転送・構築の費用も測る。SIMDは計測で優位性を示せた場合だけ採用する。
- 索引は正規化済み原文・長さ・keyを保存し、派生構造をロード時に構築する。差分サイズとロード費用はM2/M4で検証する。
