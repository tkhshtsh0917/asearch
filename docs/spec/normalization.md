# 正規化・順位・ポリシー

[仕様索引](README.md)へ戻る。節番号は分割前から維持する。

M0以後の計画を含む。実装済みの範囲は[開発計画](../PLAN.md)を参照する。

<a id="section-8"></a>

## 8. 正規化（任意の Strategy）

- 互換クラス `Asearch` では**常に無効**（asearch.ts は正規化しない）。一括 API（AsearchIndex／SearchSession／IntentIndex）の既定は `ja-basic-v1` の項目 1〜5 を有効、項目 6 を無効にする（D4）。`normalizer: 'none'` でprofile/mode/wrapを揃えると、各textへasearch.tsを適用して0..Kを順に調べた最小levelと一致する。単にmatch(text, K)を1回呼ぶフィルタとは、上流の非単調性により一致しない場合がある（§2.8）。
- 実行場所は Rust 側（索引構築＝CLI または WASM 版のビルダ、照合時＝WASM。どれも同一コード）。理由:
  1. 索引とクエリで正規化が食い違う事故を構造的に排除できる。
  2. JS の `normalize('NFKC')` は実装の Unicode／ICU の版に依存しうる。
  3. クエリは短いので処理コストは無視できる。
- 索引ヘッダに normalizer の (id, version, options) を記録し（§11.2）、実行時が対応していなければロードを拒否する。
- `ja-basic-v1`（各項目を個別に on/off できる。一括 API の既定は 1〜5 が有効、6 が無効）:
  1. U+3000 → U+0020（**WildcardSpace で全角空白をワイルドカードとして扱うために重要**）
  2. 全角 ASCII U+FF01–FF5E → U+0021–007E
  3. 半角カナ U+FF61–FF9F → 全角カナ（ﾞ・ﾟ は直前と合成できれば合成: ｶﾞ→ガ、ﾊﾟ→パ、ｳﾞ→ヴ）
  4. カタカナ U+30A1–30F6 → ひらがな（−0x60）
  5. ASCII 大文字 → 小文字（両側）
  6. 長音・ダッシュ類の統一（ー／ｰ／－／‐／―／−／ASCII `-` → U+30FC）。全角ASCII変換との設定の組合せでも冪等になるようASCIIハイフンを含む。型番検索を壊しうるので既定は off
- 正規化は長さを変えうる（ｶﾞ は 2 記号 → 1 記号）。編集距離は正規化後の記号列で数える。ハイライト用の原文 UTF-16 オフセットへの写像は、上位 N 件についてだけ再計算する。
- 完全な NFKC（unicode-normalization クレート）は任意の feature。WASM サイズの増加を計測して採否を決める。

M2のRust基盤はNormalizer::NONE/JA_BASICとfrom_descriptor(id,version,options)。descriptorは索引ヘッダと同じ(0/1,1,u32)であり、noneはoptions=0、ja-basicは0..63を受理する。未知id/versionと不正optionsを区別して拒否する。normalize_intoは入力のUTF-16単位数以上の容量を持つVecへ確保ゼロで書き込み、足りない場合は以前の出力を変更せずNeedsCapacityを返す。容量予約とinstance全体の予算管理は呼出側の索引/Workspaceが行う。孤立サロゲートは置換せず保持する。

項目3のかな合成は、直後の半角濁点/半濁点を幅変換したときに行い、その後に項目4を適用する（ワ＋ﾞ→ヷは指定のひらがな変換範囲外）。既存のU+3099/U+309Aや他の結合文字に一般的なNFCは施さない。対応表は[Unicode 17.0.0の限定データ](../../vendor/unicode/README.md)から生成し、全NFKCとは区別する。全64設定で全単位とかな/濁点の組合せ、混合列の冪等性を検査する。索引/JSへの接続はM2で続けて実装する。

---

<a id="section-9"></a>

## 9. ランキング・重複排除・ハイライト

- 順位: (level, order, key) の昇順。order は文書ごとの順位キー（小さいほど上位、既定は key）で、不変セグメントでは配置順に織り込み済みなので、抽出時にソートしない（§4.4、§11.3）。全セグメントを不変として扱う。
- 重複排除: 結果は key 単位。同じ key の text（言い換え、読み、展開文）のうち最初の（最良の）1 件だけを残す。text ごとに別の行が欲しい場合は、text ごとに別の key を振る。
- 順位だけの変更も更新（upsert）として扱う（§20.2）。大量の順位変更は compaction で反映する。
- ハイライト（`explain`）: 上位 N 件だけ、(pattern, text) の小さな DP をトレースバックする。M5実装は残り総予算内のDP行列を用い、容量不足をRESOURCE_LIMITで拒否する。原文写像はnormalize_mapped_intoで再計算し、無料loopはハイライトしない（D26）。同コストの整列が複数ある場合の選び方: 終端が最も左 → 始端が最も右（最短区間）→ 一致を優先。返り値はtext上のUTF-16オフセット区間列とcoordinateSpace。原文は呼出側のoriginalを優先し、無ければOLEN/OTXTを使う。originalを渡す場合は正規化後の内容が索引textと等しいことを検査する（不一致はINVALID_INPUT）。原文が無い場合は正規化後textとnormalized-utf16を返し、原文位置であると偽らない。IntentIndexは選ばれた展開をtextとし、その正規化前の展開へ戻せる場合だけoriginal-utf16を返す。

---

<a id="section-10"></a>

## 10. 曖昧度ポリシー（Strategy）

M2のRust基盤ではAmbigPolicy::fixed/by_lengthとLengthUnitを実装済み。descriptorは構築時に検証し、evaluateは正規化済み・wrap前のUTF-16入力から確保ゼロでKを求める。QueryWorkspaceはResourceLimitsを検証してactive/stagingの2バッファをfallibleに予約し、updateで正規化→K→wrapを一度に実行する。失敗時は前回成功したpatternとKを保持する。入力上限に内部wrapの2単位は含めない。単体Workspaceの予算は自身の容量のみで、索引全体の合算は所有indexへ接続するときに適用する。これは検索前処理であり、文書の照合やSearchSessionではない。

処理順は **正規化 → モードに従った記号化と長さ計数 → K の決定 → wrap の付加 → 照合**。wrapはWildcardSpaceでだけ適用し、Exact/Includeでは値にかかわらず補わない。既定の K = min(floor(len / 4), 3)（D2）は変えない。

- `fixed(k)` は k をそのまま使う。`byLength({ every: 4, max: 3, unit: 'profile', includeWildcards: false })` が既定。省略した後ろ 2 項目はこの値。
- `unit: 'profile'` は選択した profile の text 記号単位、`'code-point'` は code point、`'utf16'` は code unit。孤立サロゲートはどちらでも 1 と数える。
- 長さは正規化後の入力で数え、wrap が補った空白は常に除く。`includeWildcards: false` は WildcardSpace の U+0020 を除く。Exact/Include の空白はリテラルなので数える。`true` はユーザーが入力した空白も数える。
- 例（既定）: `abc` → len=3/K=0（wrap 後も K=0）、`abcd` → 4/1、`ab cd` → 4/1、`ＡＢＣＤ` → 4/1、`ｶﾞｷｸｹ` → 正規化後 `がきくけ` で 4/1。code-point profile の `😀😀` は 2/0、同じ入力を unit=utf16 で数えると 4/1。
- `every` は正の安全な整数、`max` と `fixed(k)` は 0..7 の整数。数値の `maxAmbig` も 0..7。不正値は `INVALID_INPUT` とし、丸めない。0..3 は単相化、4..7 は汎用経路（§4.5）。互換 API の ambig はこの制限を使わず §19 に従う。
- カスタムポリシーは `AmbigContext` を受ける JS 関数とし、正規化後の文字列、mode、profile を渡す。返値も 0..7 の整数として検査する。例外なら検索を中断し、セッションの前回成功状態を保つ。呼出回数は update/search につき 1 回。関数自身の確保は Rust コアの確保ゼロ保証に含めない。
- 曖昧度ポリシーの既定の根拠は D2 を参照する。既定の根拠と、len の定義を同一視しない。境界 3/4・7/8・11/12、正規化による長さ変更、wrap、mode、非 BMP を受入テストにする。

---
