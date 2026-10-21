# 拡張APIとWASM構成

[仕様索引](README.md)へ戻る。節番号は分割前から維持する。

M0以後の計画を含む。実装済みの範囲は[開発計画](../PLAN.md)を参照する。

<a id="section-12"></a>

## 12. JS/TS API

<a id="section-12-1"></a>

### 12.1 互換 API（asearch.ts@1.0.3 と同名同型）

```ts
export enum MatchMode {
  Exact = 0,
  Include = 1,
  WildcardSpace = 2,
} // 逆引き付き
export class Asearch {
  static MAXLENGTH: number; // 31
  readonly matchMode: MatchMode;
  readonly pattern: string;
  masks: { [char: string]: number }; // asearch.ts と同じ値
  constructor(
    pattern: string,
    mode?: MatchMode,
    options?: { profile?: "asearch.ts" | "asearch.ts-lib" | "ideal" },
  ); // 第 3 引数は追加（後方互換）
  match(query: string, ambig?: number): boolean;
  // 追加（非破壊）
  matchLevel(query: string, maxAmbig?: number): number; // 最初に一致する level。無ければ -1（Q3 の影響を受けない）
  free(): void;
  [Symbol.dispose](): void;
}
```

### M1: 最小levelと解放の契約

`matchLevel(query, maxAmbig = 3)`は、選択profileで0からmaxAmbigまでの最初の一致levelを返し、一致が無ければ-1。Rustの1回の走査で求め、levelごとにJSから再呼出ししない。maxAmbigは原始numberの整数0〜31で、非numberはTypeError、範囲外/非整数/非有限はRangeError。型検査順は解放状態→queryの文字列検査→maxAmbig。解放後はDISPOSED。既存matchの65,535までの互換経路と混同しない。

Rustはmatch_level/match_level_utf16でResult<Option<u16>, InvalidMaxAmbig>を返す。max_ambigは0〜31、範囲外をErrで返す。通常のmin-level評価も確保ゼロ。

M1ではoptions.profile="ideal"も実装する。pattern/textの記号はcode point（孤立サロゲートは元の値を保持）、大小文字候補・mode・引数変換・公開フィールドは既存compilerと同じ。相違はIdeal初期ε閉包のみであり、match(text,e)はcost≤eを返す。matchのlevel上限は65,535のままで、32以上でも初期ビットを折り返さず、語幅を超えるprefixは飽和させる。matchLevelの上限は全profile共通で31。Asearchのpattern上限も全profile共通で31 UTF-16単位とし、長い入力は別のRust ScalarWm APIへ渡す。正規化はこの単発APIに追加しない。

`free()`はRust側ハンドルを明示解放し、冪等とする。`[Symbol.dispose]()`は同じ操作。解放後の`match`は入力変換より先に`code: "DISPOSED"`を持つErrorをthrowする。不正receiverはTypeError。既存の公開フィールドとその列挙順は維持し、他のインスタンスへ影響しない。

Symbol.disposeが無い実行環境でもfree()を使える。ランタイムではSymbol.for("Symbol.dispose")をフォールバックキーとして定義し、グローバルSymbolを変更しない。TypeScriptで追加APIを使う場合のlibにはESNext.Disposableを含める。

<a id="section-12-2"></a>

### 12.2 JS と WASM の境界の作法

- 大文字小文字の候補キーは JS 側で `[c, c.toLocaleLowerCase(), c.toUpperCase()]` として計算し、単一記号になるものだけを WASM のパターンコンパイルに渡す（Q8 を完全一致させる）。`masks` プロパティもこの結果から asearch.ts と同じ式で作る。
- text は UTF-16 code unit のまま、事前確保した WASM 側のバッファへ書き込み、`match_u16(handle, len, ambig)` を呼ぶ（TextEncoder を使わない。孤立サロゲートも保持される）。（最終形。M0 は `&[u16]` を渡す。§19.4）
- Rust 側の pattern はハンドル（slab）で保持し、`FinalizationRegistry` と明示的な `free()` で解放する。
- 単発の `match` が JS↔WASM の越境コストに支配される可能性があるので、asearch.ts 実物と素の JS 移植との比較ベンチを必須にする。JS の高速経路を足すのは M7 の任意項目とし、同じフィクスチャで検証する。

<a id="section-12-3"></a>

### 12.3 追加 API

M2の現行JS実装はAsearchIndex.fromStrings/fromDocs/fromBytes/toBytes/search/freeとbuildIndex、fixed/byLength descriptor。bulkのprofileはIdeal/asearch.ts/asearch.ts-libを受け付ける。正規化・K・wrapをRustで準備し、JSで単発Asearchと同じlocale lower/upper候補を生成してRustへ渡す。キーの元記号・個数・範囲は照合前に検証する。JSコールバック後も解放状態を再検査し、同一indexへのsearch再入はCONFLICTで拒否する。結果は下記SearchResultの配列・length・copy・totals・statsの形で、現在はJS所有のコピーを返す（WASMビューは未実装）。statsのblocks/earlyExitBlocksは実際の全量実行から取得する。未知・未実装のoptions（osaなど）を指定した場合は拒否する。fromStreamは64Bヘッダ→固定WASMバッファへ直接chunkコピーし、await後にmemory viewを更新する。SearchSessionは独立query/workspaceとbounded row checkpointを持ち、update/reset/free・親解放時無効化を実装した。maxCacheBytes=0またはengine=transposed-blockedは全量計算を使う。stats.reusedRowsは再利用した接頭辞行数。カスタムAmbigPolicyも正規化後・内部wrap前のcontextで一度だけ同期評価し、整数0〜7を検証してから照合する。失敗時はcheckpointを書き換えず、コールバック後に解放状態を再検査する。diffManifestの検証・比較は実装済み。expectedのbytes/texts/id/SHA-384検証も共通Rust実装を使う。IndexSetを共有するWASM更新も実装し、addSegment/replaceSegments・segments・canonical compactionを提供する。成功更新で全refsを失効させ、旧検索中も輸送バッファを総予算へ含める。explainも原文写像と世代検証を実装し、同点のD26規則と残予算を守る。IntentIndexもD27の範囲で実装済み。未採用のengineはUNSUPPORTED_OPTIONで拒否する。以下の最終API案全体を実装済みとは扱わない。

```ts
export function init(
  module?: WebAssembly.Module | BufferSource | URL,
): Promise<void>;
// 既定の ESM エントリは top-level await で自動初期化する。CJS エントリは同期コンパイル（Node）。

export interface ResourceLimits {
  maxTotalBytes?: number;
  maxTexts?: number;
  maxUnits?: number;
  maxSlots?: number;
  maxWorkingBytes?: number;
  maxPatternUnits?: number;
  maxSegments?: number;
}
export type AmbigPolicy =
  | { kind: "fixed"; value: number }
  | {
      kind: "by-length";
      every: number;
      max: number;
      unit?: "profile" | "code-point" | "utf16";
      includeWildcards?: boolean;
    }
  | ((context: AmbigContext) => number);
export interface AmbigContext {
  normalized: string;
  mode: MatchMode;
  profile: "asearch.ts" | "asearch.ts-lib" | "ideal";
}
export function fixed(k: number): AmbigPolicy;
export function byLength(options?: {
  every?: number;
  max?: number;
  unit?: "profile" | "code-point" | "utf16";
  includeWildcards?: boolean;
}): AmbigPolicy;
export type EngineName =
  | "scalar-wm"
  | "transposed-blocked"
  | "incremental"
  | "narrowing"
  | "dag"
  | "simd-lanes"
  | "myers";
export interface NormalizerConfig {
  id: "none" | "ja-basic";
  version: number;
  options: number;
}
export interface SearchStats {
  engine: EngineName;
  segments: number;
  blocks: number;
  earlyExitBlocks: number;
  elapsedMs?: number;
}
export interface IndexOptions {
  profile?: "asearch.ts" | "asearch.ts-lib" | "ideal";
  normalizer?: "none" | "ja-basic-v1" | NormalizerConfig; // 既定 'ja-basic-v1'（項目 1〜5、D4）
  blockWords?: number; // 正の安全な整数。実際のレイアウトは資源予算内。索引ファイルには保存しない
  limits?: ResourceLimits;
}
export interface Doc {
  key: number;
  order?: number;
  texts: readonly string[];
} // order の既定は key（§20.2）
export interface LoadOptions extends IndexOptions {
  maxBytes?: number; // 既定 256 MiB。ファイル単体の上限（§11.6）
  expected?: ManifestSegment; // あれば bytes/texts/id/integrity を構築・採用前に照合（§20.5）
}
export interface SearchOptions {
  mode?: MatchMode; // 一括 API の既定は WildcardSpace（D5）
  wrap?: boolean; // WildcardSpace で pattern の前後に空白を補う（re_expand や takker のエンジンと同じ ` q ` 慣行）。既定 true（D5）
  maxAmbig?: number | AmbigPolicy; // 既定 byLength({ every: 4, max: 3 })（D2）
  limit?: number; // 既定 30（takker のエンジンの既定に合わせた暫定値）
  engine?: "auto" | EngineName;
  countTotal?: boolean;
  osa?: boolean;
}
export interface SearchResult {
  ids: Uint32Array; // key。結果は key 単位で (level, order, key) の昇順（§9）。WASM メモリのビュー: 次の呼び出しか更新まで有効
  levels: Uint8Array; // 同上。copy() で複製できる
  refs: Uint32Array; // 一致した text の参照。ビューと参照値の寿命を分ける（§12.4）
  orders: Uint32Array; // 各結果の order（mergeResults での併合に使う）
  length: number;
  copy(): SearchResult; // 型配列をJS所有メモリへコピー。コピーのcopy()も独立したコピーを返す
  totals?: Uint32Array; // countTotal=true のとき K+1 要素。最小level別の異なるkey数（§4.4）
  stats: SearchStats; // 使用エンジン、セグメント数、ブロック数、早期終了数、時間（任意）
}
export interface SegmentInfo {
  id: string;
  texts: number;
  dead: number;
  bytes: number;
} // id = ヘッダの file_xxh3（16 進 16 桁）
export class AsearchIndex {
  static fromStrings(
    texts: readonly string[],
    options?: IndexOptions,
  ): AsearchIndex; // key = 添字
  static fromDocs(docs: Iterable<Doc>, options?: IndexOptions): AsearchIndex;
  static fromBytes(bytes: Uint8Array, options?: LoadOptions): AsearchIndex;
  static fromStream(
    body: ReadableStream<Uint8Array>,
    options?: LoadOptions,
  ): Promise<AsearchIndex>;
  // セグメントと更新（§20）
  addSegment(
    source: Uint8Array | ReadableStream<Uint8Array>,
    expected?: ManifestSegment,
  ): Promise<SegmentInfo>; // 古い順に追加。TOMB を既存セグメントに適用
  replaceSegments(
    sources: readonly {
      source: Uint8Array | ReadableStream<Uint8Array>;
      expected?: ManifestSegment;
    }[],
  ): Promise<void>; // すべて構築してから切り替える
  segments(): SegmentInfo[];
  toBytes(): Uint8Array; // 生きている文書を 1 セグメントに作り直したバイト列（決定的。compaction と同じ）
  search(pattern: string, options?: SearchOptions): SearchResult;
  explain(
    ref: number,
    pattern: string,
    options?: SearchOptions & { original?: string },
  ): // M5。原文は OLEN／OTXT か original（§11.2）
  {
    text: string;
    level: number;
    spans: [number, number][];
    coordinateSpace: "original-utf16" | "normalized-utf16";
  } | null;
  session(options?: SearchOptions & { maxCacheBytes?: number }): SearchSession; // 既定32MiB、総予算内
  free(): void;
}
export function buildIndex(
  docs: Iterable<Doc>,
  options?: IndexOptions,
): Uint8Array; // サーバー（Node）用。CLI と同じバイト列
export interface ManifestSegment {
  id: string;
  url: string;
  bytes: number;
  texts: number;
  integrity: string;
}
export interface Manifest {
  format: "asearch-manifest/1";
  indexId: string;
  generation: number;
  index: {
    major: number;
    minor: number;
    symbol_unit: "code-point" | "utf16";
    normalizer: { id: string; version: number; options: number };
  };
  segments: ManifestSegment[];
}
export function diffManifest(
  current: Manifest | undefined,
  manifest: Manifest,
):
  | { kind: "stale" }
  | { kind: "none" }
  | { kind: "append"; add: ManifestSegment[] }
  | { kind: "replace"; segments: ManifestSegment[] }; // §20.5
export function mergeResults(
  parts: readonly SearchResult[],
  limit: number,
): // 静的な Worker 分割（§12.3 の注）の結果を併合
{
  ids: Uint32Array;
  levels: Uint8Array;
  orders: Uint32Array;
  parts: Uint8Array;
  refs: Uint32Array;
}; // (level, order, key) 順、key で重複排除。refs は parts が示す元の結果の中でだけ有効
export class SearchSession {
  update(pattern: string): SearchResult;
  reset(): void;
  free(): void;
}
export class IntentIndex {
  static fromEntries(
    entries: readonly { expr: string; key: number; order?: number }[],
    options?: IndexOptions,
  ): IntentIndex;
  search(pattern: string, options?: SearchOptions): SearchResult;
  explain(
    ref: number,
    pattern: string,
    options?: SearchOptions,
  ): {
    text: string;
    level: number;
    spans: [number, number][];
    coordinateSpace: "original-utf16" | "normalized-utf16";
  } | null;
  free(): void;
}
```

- `process` などの Node 専用グローバルを参照しない（Q6）。
- 入力の防振、latest-wins、requestAnimationFrame での合流、DOM の更新は範囲外（§0）。README に推奨手順（時間ベースの debounce を既定にしない、最新要求だけを処理する、描画は rAF で合流する）を書くだけで、API は持たない。
- `mergeResults`はstructured clone可能なids/levels/orders/refs/lengthの部分型も受け付ける。最大256 partsで、全partの外部プロパティ取得後に実要素型・長さ・昇順を検査する。共有bufferの並行書換えは呼出側で防ぐ。配列長はTypedArrayの組み込みgetterで取得し、独自getterやサブクラスによる偽装を採用しない。出力型配列の予約上限は256MiB（JS heap全体の保証ではない）。同順位の同keyは入力part順・part内順で決定し、totalsは返さない。
- Worker 並列化は SharedArrayBuffer（COOP/COEP が必要で、埋め込みウィジェットでは使えないことが多い）に頼らず、**セグメント（またはブロック範囲）を分けて各 Worker が別々に保持**し、上位 N を `mergeResults` で (level, order, key) の順に併合する（索引の総量は増えないが、Worker ごとの Workspace とキャッシュは増える）。Worker の生成とメッセージングは呼び出し側が行う。
- **v1 では、Worker 分割は更新しない索引（静的な分割）に限る。** 更新（tombstone）を使う場合は、1 つの Worker に索引全体を持たせる（別の Worker にあるセグメントへ tombstoneを届ける規則は定めない。§20.8）。

---

<a id="section-12-4"></a>

### 12.4 エラー・更新の直列化・借用の寿命（M2 以降）

- bulk API の入力検査はメソッド呼出時に行う。同期メソッドは `Error` を throw、非同期は reject し、いずれも安定した `code` を付ける。コードは `INVALID_INPUT`、`INVALID_INDEX`、`UNSUPPORTED_FORMAT`、`UNSUPPORTED_OPTION`、`RESOURCE_LIMIT`、`INVALID_MANIFEST`、`CONFLICT`、`BUSY`、`DISPOSED`。Rust は対応する型付き `Result` を返す。メッセージ文字列を機械判定に使わない。型不正は`TypeError`に`INVALID_INPUT`を付ける。呼出側のgetter・policy callback・streamが投げた例外は元のまま伝播し、診断codeを保証しない。互換 `Asearch.match` の例外は §19 を維持する。
- 選択できるengineは実装済みかつ当該APIの前提を満たすものだけ。Fixed指定が使えない場合は `UNSUPPORTED_OPTION`（autoだけが安全なfallbackを選ぶ）。M0にbulkの型やstubを先取りしない。NormalizerConfigはnone/version=1/options=0、ja-basic/version=1/options=0..63のみをv1で受理し、未知versionは `UNSUPPORTED_FORMAT`、不正optionsは `INVALID_INPUT`。検索の経過時間はJS/CLI側で取得し、コアはtime APIに依存しない。
- key/order は 0..2^32−1 の整数（NaN・小数・負数・範囲外を拒否）。text は原始文字列、文書の texts は 1 個以上。各textは正規化前/後の両方で65,535 UTF-16 code unit以下とし、OLEN/TLENへ切り詰めず格納できることを検査する。同一 key の複数文書入力をエラーにし、同じ文書内の text 順は意味のある入力として保存する。limit は 0..maxTexts の整数で 0 も可。空 pattern は暗黙に抑制せず、選んだ意味論で評価する。
- 同じ index の非同期更新は最大 1 操作。2 個目の更新は `BUSY`。検索は旧 active で続けられる。free は staging を中止し、後着の完了は採用しない。失敗/中止で旧状態を保持する。異なる instance 間の並行は可。
- 結果の型配列は常にJS所有の独立したコピーであり、次のAPI呼出し・索引更新・`free()`後も内容を保持できる。WASMメモリへの借用ビューではない。`.copy()`は型配列とstatsをさらに独立して複製する。配列内のopaque refsは更新後に失効する。JSとWASM境界の確保は、準備済みRustコアの確保ゼロとは区別する。
- refs の整数値は、その index の更新世代に属する opaque な参照。配列をcopyしても値は更新成功後に失効し、他 index/Worker にそのまま渡せない。explain は世代と範囲を検査し古い参照を `INVALID_INPUT` とする。世代を含む参照表は予約した容量内で管理し、番号の再利用で古いrefが別textを指さないようにする。参照番号空間を使い切ったら `RESOURCE_LIMIT` とし、黙って折り返さない。
- free は冪等。解放済み index/session の利用は `DISPOSED`。indexを解放すると子sessionも無効。更新成功時は影響する cacheだけを無効化し、検索オプションと入力を保持する。

---

<a id="section-13"></a>

## 13. WASM ビルド構成

- `cargo build --target wasm32-unknown-unknown --release` → `wasm-bindgen`（CLI を crate と完全に同じ版へ固定）→ `wasm-opt -O3`（Binaryen）。
- wasm-packには依存せず、Cargo.lockと同じ版のwasm-bindgen CLIを直接使う。
- SIMDを採用した場合の成果物は 2 つ: `*_simd.wasm`（`-C target-feature=+simd128`）と基本版。実行時に小さな SIMD モジュールを `WebAssembly.validate` して選ぶ。SIMD のコードは `#[cfg(target_feature = "simd128")]` で分岐する（関数単位の `#[target_feature]` は使わない）。relaxed-simd は採用しない。
- Rust 1.87 以降で既定有効の WASM 機能: multivalue、mutable-globals、reference-types、sign-ext、nontrapping-fptoint、bulk-memory。
- `[profile.release]`: `opt-level = 3`、`lto = "fat"`、`codegen-units = 1`、`panic = "abort"`、`debug = false`、`strip = true`。
- アロケータはRustの既定を使う。準備済みbulkの検索中には確保しない。DAGや説明など確保を行う経路は、個別の資源予算で検証する。
- CJSはNodeの同期初期化、ブラウザは非同期のWASM取得・初期化を使う。基本命令版を配布し、任意engineの採否はM7で判断する。
- サイズ: raw／gzip／brotliをCIで記録し、差分をレビューする。速度の回帰判定は§15.1.1の別手順。
- 安全性: 本体 `asearch-rs` は全マイルストーンで `#![forbid(unsafe_code)]`。little-endian の索引は検査済みsliceから明示的に復号し、ファイルの型付きpointer castを前提にしない。M7でunsafeなSIMD intrinsicが必要なら、小さな任意kernel crateへ隔離し、境界・整列・長さを検査したsafe APIのみをcoreから呼ぶ。M0にkernel crateを作らない。
- M0のJS formatter/linterはBiome 2.5.15（D17）をpackage.jsonとlockで固定する。Rust/JSのformat/lintとtscの型検査を別コマンドにし（webはDOM lib、WorkerはWebWorker libで独立に検査。実行時DOM非依存と型ライブラリの指定を混同しない）、formatによる修正とcheckを分ける。上流コピーの原本を一括formatしない。

---
