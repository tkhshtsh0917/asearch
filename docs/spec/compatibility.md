# M0互換APIと実装契約

[仕様索引](README.md)へ戻る。節番号は分割前から維持する。

M0で実装済み。互換の規範として維持する。

<a id="section-19"></a>

## 19. 初期移植（M0、initial commit）の詳細

方針: asearch.ts@1.0.3 の挙動と公開 API を、Rust ＋ WASM で**そのまま**再現する。本書の改善（§3〜§11、§20）は入れない。レビューしやすいよう、Rust 側の遷移コードは asearch.ts の `match()` と 1 対 1 に対応させる（MSB 先頭のビット順、同じ変数の役割）。

**優先順位**: M0 については本節が §5.2・§12・§13・§14 より優先する（それらは最終形の記述）。本節に書いていないことは、§2 と同梱の上流 `vendor/upstream/`（ネットワーク無しで使える互換の正）の挙動に従う。それでも決まらなければ [DECISIONS](../DECISIONS.md) に書いて質問する。

<a id="section-19-1"></a>

### 19.1 M0 で作るもの

```text
Cargo.toml（workspace）, rust-toolchain.toml, mise.toml（Node/pnpm。Rust版の正本はrust-toolchain.toml）, .gitignore, README.md, LICENSE-MIT, LICENSE-APACHE, NOTICE
package.json（root は "type": "commonjs"。tools/reference/*.js と同梱upstreamのCommonJSを保つ）, pnpm-lock.yaml（pnpm workspaces: packages/asearch-wasm と tools/fixtures。devDependencies は typescript・esbuild・binaryen・@playwright/test と @biomejs/biome をルートに置く。lock はコミットする）
crates/asearch-rs/      src/lib.rs ほか、tests/compat.rs（フィクスチャ照合）、tests/alloc.rs（計数アロケータ）
crates/asearch-wasm/    src/lib.rs（wasm-bindgen の境界だけ）
packages/asearch-wasm/  package.json, src/*.ts, test/*.test.mjs（node:test）, test/types/（d.ts 互換）
tests/browser/          Playwrightの設定・テスト・server.mjs・fixtures/
tools/fixtures/         package.json（typescript を固定）, gen.mjs
crates/asearch-rs/tests/fixtures/         compat.json（生成物。コミットする）
.github/workflows/ci.yml
```

作らないもの: `asearch-cli`、`tools/bench`、ベンチ、SIMD 版、§3.2 の trait 群。`.gitignore` には `target/`、`node_modules/`、`packages/asearch-wasm/dist/`、`__pycache__/`、`*.pyc` を入れる（`dist/` はコミットしない。CI と `npm pack` で作る）。

<a id="section-19-2"></a>

### 19.2 版（2026-10-20 00:43〜01:05 JST にコマンド出力で確認した値）

作業環境で `--version` を取り直し、違う版にする場合は理由を [DECISIONS](../DECISIONS.md) に書く。

| 項目                             | 値                                                                                                                                                                         | 確認に使ったコマンド                                                              |
| -------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------- |
| Rust                             | 1.97.0。`rust-toolchain.toml` は `channel = "1.97.0"`、`components = ["rustfmt", "clippy"]`、`targets = ["wasm32-unknown-unknown"]`。edition 2024、`rust-version = "1.97"` | `rustc --version`                                                                 |
| wasm-bindgen と wasm-bindgen-cli | `=0.2.129`（Cargo.toml で `=` 固定。CLI も同じ版を `cargo install wasm-bindgen-cli --version 0.2.129 --locked`）                                                           | `cargo search`、`cargo info`                                                      |
| binaryen（wasm-opt）             | npm `binaryen@132.0.0` を devDependency にして `pnpm exec wasm-opt`。この版に `tools/build/build.mjs` の `--enable-*` がすべてあることを `wasm-opt --help` で確認済み      | `npm view`、`wasm-opt --version`（version_132）                                   |
| Node                             | 開発用はmiseで24.22.0。CIは22.22.0と24.22.0の2系列（D19）                                                                                                                  | `node --version`                                                                  |
| TypeScript                       | 製品の型検査は7.0.2。上流fixtureの変換だけはtools/fixturesで5.9.3に隔離する（D19）                                                                                         | `npm view typescript@7.0.2 version`、`tsc --version`                              |
| @playwright/test                 | 1.64.1（M0 は Chromium だけ）                                                                                                                                              | `npm view @playwright/test version`                                               |
| esbuild                          | 0.28.2（TS ラッパから ESM と CJS の両方を作るため。§19.5）                                                                                                                 | `npm view esbuild version`（2026-10-20 01:34 JST）                                |
| JS formatter/linter              | `@biomejs/biome@2.5.15`（ルートdevDependency、生成物・上流コピーを除外）                                                                                                   | `npm view @biomejs/biome version --json` → `"2.5.15"`（2026-10-20 JST、今回確認） |
| テストランナー                   | Rust は `cargo test`、JS は Node 組み込みの `node:test`（追加の依存なし）                                                                                                  | —                                                                                 |

- LLVM 20 以降の Rust は `bulk-memory-opt` や `call-indirect-overlong` といった細かい機能も出力に使う可能性がある（未検証）。`wasm-opt` が「許可されていない機能」で失敗したら、出力の target_features を確認して、足りないフラグだけを足す（`--all-features` は使わない）。

<a id="section-19-3"></a>

### 19.3 Rust（`crates/asearch-rs`）

```rust
pub const MAX_LENGTH: usize = 31;
pub const MAX_AMBIG: u16 = u16::MAX;   // 65,535。JS 境界はこれを超えたら RangeError（§19.5、D10）
#[repr(u32)] pub enum MatchMode { Exact = 0, Include = 1, WildcardSpace = 2 }
pub enum Profile { AsearchTs, AsearchTsLib }   // 既定 AsearchTs（D1）
pub struct PatternTooLong { pub pattern: String }   // Display: "Pattern must be shorter than 32 chars. But given {pattern}"

impl Asearch {
    /// Rust 単体で使う入口。pattern の UTF-16 長（空白を含む）が 31 を超えたら Err（Q2）
    pub fn new(pattern: &str, mode: MatchMode) -> Result<Self, PatternTooLong>;
    pub fn with_profile(pattern: &str, mode: MatchMode, profile: Profile) -> Result<Self, PatternTooLong>;
    /// JS 境界の入口。asearch.ts と同じ JS コードで作った状態をそのまま受け取る（§19.5）。
    /// masks は (code point, mask) の組で、1 code point のキーだけ。init は initPattern
    pub fn from_parts(masks: &[(u32, u32)], loop_mask: u32, accept: u32, init: u32, profile: Profile) -> Self;
    pub fn is_match(&self, text: &str, ambig: u16) -> bool;          // Rust の match は予約語なので is_match
    pub fn is_match_utf16(&self, text: &[u16], ambig: u16) -> bool;  // WASM 経路
}
```

- 遷移は §2.3の遷移どおり。**シフトは JS の `>>>`／`<<` と同じく量を 32 で割った余りにする**（`u32::wrapping_shr`／`wrapping_shl`）。Rust の `>>` は量が 32 以上だと debug ビルドで panic するので使わない（ambig 32・33 のフィクスチャで落ちる）。
- text の走査: `AsearchTs` は code point（正しいサロゲート対は 1 記号、孤立サロゲートはその code unit の値を 1 記号）、`AsearchTsLib` は UTF-16 code unit。`is_match(&str)` の `AsearchTsLib` は UTF-16 に符号化して単位で走査する。
- マスク表: キーは最大 93 個なので、code point 昇順のソート済み配列を二分探索（または線形探索）で引く。65,536 要素の表を作らない（§5.2 の世代スタンプ付き直接表は M1 以降の最終形）。非 BMP の記号でも panic しない。
- `new`（Rust 単体）の大文字小文字: asearch.ts の `[c, c.toLocaleLowerCase(), c.toUpperCase()]` の代わりに `char::to_lowercase`／`to_uppercase` を使い、結果が 1 文字のものだけをキーにする。複数文字になるキー（`SS`、`i̇` など）は 1 記号の text と一致しえないので、落としても照合結果は変わらない。ロケール依存の差（トルコ語の İ など）がありうることを doc に書く。
- 状態配列: `ambig ≤ 31` は固定長配列（`[u32; 32]`）で確保なし。`32 ≤ ambig ≤ 65,535` は asearch.ts と同じく `ambig + 1` 要素の一時バッファ（最大 256 KiB）を確保してよい（互換のためだけの経路で、性能の対象外）。
- `#![forbid(unsafe_code)]` はこの crate の lib にだけ付ける。`crates/asearch-wasm`（wasm-bindgen の生成コードが unsafe な属性を使う）と、integration test の計数アロケータ（`unsafe impl GlobalAlloc`）は対象外。

<a id="section-19-4"></a>

### 19.4 WASM 境界（`crates/asearch-wasm`）

```rust
#[wasm_bindgen] pub struct AsearchHandle(asearch_rs::Asearch);
#[wasm_bindgen] impl AsearchHandle {
    #[wasm_bindgen(constructor)]
    pub fn new(key_cps: &[u32], key_masks: &[u32], loop_mask: u32, accept: u32, init: u32, profile: u8)
        -> Result<AsearchHandle, JsError>;   // 配列長が違う・profile が 0／1 以外なら Err（panic しない）
    pub fn match_u16(&self, text: &[u16], ambig: u16) -> bool;
}
```

- ロジックは置かない。text は UTF-16 code unit のまま受け取る（TextEncoder を使わない）。
- M0 では `&[u16]` の受け渡し（wasm-bindgen が呼び出しごとに WASM メモリへコピーし、確保もする）で良い。M0 の「確保ゼロ」はネイティブの `is_match` だけが対象。事前確保バッファへの直接書き込みは M1 のベンチで判断する。
- ハンドルの解放は FinalizationRegistry に任せる（明示 `free()` は M1）。GCの時刻や即時解放は保証しない。

<a id="section-19-5"></a>

### 19.5 JS／TS（`packages/asearch-wasm`）

- **コンストラクタは asearch.ts@1.0.3 の `lib/index.js` の本体を JS でそのまま再現する**: 長さ検査とメッセージ（`Asearch.MAXLENGTH` を毎回参照）、`matchMode` に生の `mode` を保存（`undefined` のときだけ 0）、`pattern`、`Array.from(pattern)`、`leftBit = 1 << Asearch.MAXLENGTH`、`initPattern`、`acceptPattern`、`loopMask`、`[c, c.toLocaleLowerCase(), c.toUpperCase()]` による `masks`。インスタンスの自前プロパティは名前・順序・値が asearch.ts と同じ（観測した `Object.keys` の順: `loopMask, masks, matchMode, pattern, initPattern, acceptPattern`）。
- WASM に渡すもの: `masks` のうちキーがちょうど 1 code point のもの（`[...key].length === 1`）を `(key.codePointAt(0), masks[key] >>> 0)` として、`loopMask >>> 0`、`acceptPattern >>> 0`、`initPattern >>> 0`、profile。こうすると mode の解釈（`mode === 1` のときだけ Include、`'1'`・`null`・`NaN` は Exact、`1.0` は Include。観測済み）も、大文字小文字の候補も、`MAXLENGTH` の書き換えも、asearch.ts と構成上一致する。
- 型の検査（逸脱、D10）: `pattern` と `query` は文字列か String オブジェクト（`String.prototype.valueOf`で内部スロットを確認。別realmも受理し、toStringTagの偽装は拒否する）だけを受け付け、それ以外は TypeError。String オブジェクトの pattern は、asearch.ts と同じくオブジェクトのまま `pattern` プロパティに入れ、上のコンストラクタ本体をそのまま適用する（`length` と `Array.from` が文字列と同じに働く）。query は `String(query)` で原始値にしてから UTF-16 で渡す。asearch.ts は数値や配列も受け付ける（`new Asearch(123)` は空パターン、`match(123)` は空文字列として扱い、配列は要素ごとに走査する。観測済み）。
- `match(query, ambig = 0)` の ambig は、asearch.ts の `Array.from({ length: ambig + 1 })` と `states[ambig]` の挙動を配列を作らずに再現する:
  ```js
  if (ambig === undefined) ambig = 0;
  const len = toLength(ambig + 1); // ECMAScript の ToLength(ToNumber(x))。BigInt・Symbol はこの式で TypeError（asearch.ts と同じ）
  if (len > 2 ** 32 - 1) throw new RangeError("Invalid array length"); // asearch.ts（V8）と同じ型。メッセージはエンジン依存
  checkQueryType(query); // D10 の TypeError。asearch.ts が query に初めて触れるのはここ（ambig + 1 と配列長の後、ToPropertyKey(ambig) の前）
  const key = toPropertyKey(ambig);
  if (typeof key === "symbol") return false; // "length" は asearch.ts では内部配列の長さを読む偶然の挙動になる。本実装は false（逸脱、D10）
  const i = /^(0|[1-9][0-9]*)$/.test(key) ? Number(key) : -1; // 配列添字として正規の文字列だけが states[ambig] に当たる
  if (i < 0 || i >= len) return false; // "01"・" 1"・1.5・-1・NaN・null・true は false（観測済み）
  if (i > 65535) throw new RangeError("asearch-wasm: ambig must be <= 65535"); // 逸脱（D10）
  return handle.match_u16(utf16(query), i); // "1" と [1] は ambig 1、"0" と [0] は ambig 0 として扱われる（観測済み）
  ```
- `process` など Node 専用グローバルを、ブラウザに届くコード（共通部分と web 向けエントリ）で参照しない（Q6）。`verbose` は実装しない。Node 専用エントリは `fs` などの組み込みを使ってよい。
- 構築後にインスタンスのプロパティ（`masks` など）を書き換えても照合には反映しない（asearch.ts は毎回読み直す。逸脱、D10）。
- `MatchMode` は asearch.ts と同じ形のオブジェクト（キー順 `0, 1, 2, Exact, Include, WildcardSpace`、書き込み可・列挙可）。`static MAXLENGTH = 31`（書き込み可）。
- 第 3 引数 `options.profile`: `undefined` か `'asearch.ts'` → `AsearchTs`、`'asearch.ts-lib'` → `AsearchTsLib`、それ以外は TypeError（JS で検査してから WASM に 0／1 を渡す。wasm-bindgen の `u8` は 256 を 0 に丸めるので、Rust 側の検査だけに頼らない）。
- 配布物の配置（`dist/` はコミットしない）:
  ```text
  dist/web/   wasm-bindgen --target web    の出力（asearch_wasm.js, asearch_wasm_bg.wasm）
  dist/node/  wasm-bindgen --target nodejs の出力（0.2.129 は require・exports・__dirname を使う CommonJS の .js を出す。
              パッケージは "type": "module" なので、ビルドで dist/node/package.json に {"type": "commonjs"} を書く。
              書かないと Node 22 で ReferenceError: __dirname is not defined になる。レビュー用エージェントが模擬で確認）
  dist/web.mjs   ブラウザとバンドラ向け。読み込み時に dist/web の .wasm を import.meta.url 基準で初期化する（top-level await。
                 失敗しても import 自体は失敗させず、ready を reject する）。init(input?) で別の入力から初期化し直せる
  dist/node.mjs  Node（ESM）向け。dist/node の CJS グルーを default import で受ける（名前付き import の検出に頼らない。同期初期化）
  dist/node.cjs  Node（CJS）向け（同期初期化）
  dist/index.d.ts, dist/index.d.cts   同じ内容。Asearch・MatchMode に加え init(input?): Promise<void> と可変export ready: Promise<void> を宣言する
                 （Node 向けでは init は何もせず解決、ready は解決済み）。初期化前に new Asearch() したら Error を投げる
  ```
  ```json
  "type": "module",
  "exports": { ".": {
    "node":    { "import":  { "types": "./dist/index.d.ts",  "default": "./dist/node.mjs" },
                 "require": { "types": "./dist/index.d.cts", "default": "./dist/node.cjs" } },
    "default": { "types": "./dist/index.d.ts", "default": "./dist/web.mjs" } } }
  ```
- ビルド: `src/` の TS を esbuild 0.28.2 で `dist/web.mjs`（format esm）、`dist/node.mjs`（esm）、`dist/node.cjs`（cjs）に束ね、wasm-bindgen のグルー（`./web/asearch_wasm.js`、`./node/asearch_wasm.js`）は external にする。型は `tsc --emitDeclarationOnly` で `dist/index.d.ts` を作り、`index.d.cts` に複写する。
- d.ts の互換テスト（`test/types/`、`tsc --noEmit --strict`）は**片方向**で、次の 3 つが通ること:
  ```ts
  const i: Pick<Up.Asearch, keyof Up.Asearch> = new Ours.Asearch("a"); // keyof は private を含まない
  const c: {
    new (
      pattern: string,
      mode?: Up.MatchMode,
    ): Pick<Up.Asearch, keyof Up.Asearch>;
    MAXLENGTH: number;
  } = Ours.Asearch;
  const m: Up.MatchMode = Ours.MatchMode.Include; // MatchMode は両方向に代入できる
  ```
  本パッケージの型を上流の `Up.Asearch` 全体や `typeof Up.Asearch` に代入することはできない（上流の private メンバ `acceptPattern`・`initPattern`・`loopMask`・`verbose` のため。TS2739／TS2419）。逆方向はM0の共通公開面の型に限る。M1のfree/dispose等を含む本パッケージのAsearch全体へUp.Asearchを代入する互換性は保証しない。いずれも 2026-10-20 に TypeScript 5.9.3 と 6.0.3 で確認した。

<a id="section-19-5-1"></a>

### 19.5.1 初期化の状態と再試行（M0、D11）

ビルド時に最適化済みWASMからexport名・kindの期待値を生成する。web/Nodeの生成グルーは、受け取ったWASMをキャッシュする前に期待するexport構成を検査する。不一致は初期化失敗とし、webでは正しいWASMで再試行できる。wasm-bindgen更新でガード挿入位置が変わった場合はビルドを失敗させて見直す。検査対象はexport構成であり、配信物の署名・ハッシュ検証は含まない。

状態は `uninitialized → initializing → ready | failed`。webエントリは評価時に既定入力で自動初期化を開始し、`export let ready: Promise<void>` をその試行に束縛する。top-level await はその成功/失敗処理まで待つが、失敗をmodule評価の失敗へ伝播させない。元のreadyに直ちに内部catchを付けて未処理rejectionを防ぎ、**ready自身は失敗時rejectのまま**にする。

| webでの呼出し                              | 挙動                                                                                     |
| ------------------------------------------ | ---------------------------------------------------------------------------------------- |
| uninitialized/failedで `init(input?)`      | 新しい試行を開始しreadyをそのPromiseへ差し替え、そのPromiseを返す                        |
| initializingで引数なしの `init()`          | 同じ試行のPromiseを返す（別の初期化を開始しない）                                        |
| initializingで明示入力ありの `init(input)` | code=`INITIALIZATION_IN_PROGRESS` のErrorでreject。現在の試行/readyを変えない            |
| readyで `init()`                           | 現在の解決済みreadyを返す                                                                |
| readyで明示入力ありの `init(input)`        | code=`ALREADY_INITIALIZED` のErrorでreject。既存ハンドルを無効にする再初期化は提供しない |

- `input` がundefinedなら引数なしと同じ。入力型は §12.3 の `WebAssembly.Module | BufferSource | URL`。初期化エラーはcode=`INITIALIZATION_FAILED`のErrorにcauseを保持する。失敗した試行を成功状態として採用しない。initのオーケストレーターは試行を一意に管理し、後着の古い完了で状態を書き換えない。
- readyは「現在/最後の試行」を示すlive binding。`const first = ready`のように保持した古いPromiseは差し替え後も変化しない。再試行する利用者は `await init(input)` を待つ。コンストラクタはready状態だけで利用でき、そうでなければ初期化前のErrorを投げる。
- webエントリの動的importは初回の自動試行が完了するまで待つ。初回試行の途中で同じmoduleのinitを呼べるとは約束しない。URL取得の停止やCSPエラーは失敗系のテストで扱い、別入力での再試行は失敗後に行う。
- Node ESM/CJSはローカル成果物の同期初期化。成功時readyは解決済み、init(input?)は引数にかかわらずそれを返すno-op。ローカルWASM欠損/破損時はimport/requireを失敗させる。web向けのネットワーク再試行契約をNodeへ持ち込まない。
- M0テスト: web初回成功/404/破損WASM、失敗→明示入力で成功、再試行中の同時init、成功後の再初期化拒否、古いreadyの不変性、未処理rejectionなし、ESM/CJS Node成功と成果物欠損。新しい数値計測を伴わない状態遷移の規範である。

<a id="section-19-6"></a>

### 19.6 フィクスチャ（`crates/asearch-rs/tests/fixtures/compat.json`）

- 生成は `tools/fixtures/gen.mjs`。入力は `vendor/upstream/`（先に `SHA256SUMS` を検査する）だけで、ネットワークを使わない。期待値は 3 種:
  - `lib`: 公開物 `asearch.ts/lib/index.js`（`AsearchTsLib` の正）
  - `src`: `asearch.ts/src/index.ts` を TypeScript 5.9.3 の `transpileModule`（target ES2020、module CommonJS）で変換して実行した結果（`AsearchTs` の正。`tools/reference/src_oracle.cjs` と同じ方法）。確認済み: BMP だけのケース 2,000 件で src と lib の不一致 0、非 BMP を含む 2,000 件で src と `ref.py` の `AsearchTs(unit_iter=False)` の不一致 0（ambig 0, 1, 2, 3, 32, 33。2026-10-20 01:07 JST）
  - `node`: `asearch/index.js`（ambig 0..3 だけ。M7 の NodeAsearch 用。M0 では使わない）
- 環境: `LANG=en_US.UTF-8 LC_ALL=en_US.UTF-8` で生成し、`Intl.DateTimeFormat().resolvedOptions().locale` をファイルに記録する。`en-US` 以外なら生成を止める（`İ` の小文字化がロケールで変わる。観測: tr-TR では `new Asearch('İ').match('i')` が true、en-US では false）。
- 形式（`schema: 1`）。文字列はすべて UTF-16 code unit の数値配列で持つ（孤立サロゲートを JSON の文字列にすると Rust 側で読めないため）:
  ```text
  { "schema": 1, "generator": "tools/fixtures/gen.mjs", "locale": "en-US",
    "upstream_sha256": { "./asearch.ts/lib/index.js": "<sha256>", "./asearch.ts/src/index.ts": "<sha256>", "./asearch/index.js": "<sha256>" },
    "seed": <整数>, "ambigs": [0, 1, 2, 3, 4, 5, 31, 32, 33], "node_ambigs": [0, 1, 2, 3],
    "cases": [ { "id": "<一意な名前>", "pattern": [<u16>...], "mode": { "json": <0|1|2|3|"1"|null> } | { "undefined": true } | { "nan": true },
                 "ctor": { "ok": { "masks": [[[<u16>...], <num>], ...],   // Object.keys の順
                                   "loopMask": <num>, "initPattern": <num>, "acceptPattern": <num> } }
                       | { "throws": { "name": "Error", "message": "<文字列>" } },
                 "texts": [[<u16>...], ...],
                 "lib": [[<bool> × ambigs]...], "src": [[<bool> × ambigs]...], "node": [[<bool> × node_ambigs]...] } ] }
  ```
  - `<num>` は JS の値そのまま（`acceptPattern` は空パターンで 2147483648、`' '` の WildcardSpace で −2147483648 になるので、値域は [−2^31, 2^32)）。読む側は ECMAScript の ToUint32 で u32 にする（Rust では i64 として読み `as u32`）。
  - ファイルは「1 行に 1 ケース」の JSON（`cases` の各要素を `JSON.stringify` して改行で区切る。差分が読みやすく、整形より小さい）。Node の版や時刻など、実行環境で変わる値は入れない（`--check` のバイト一致が Node 22 と 24 で崩れないように）。
- ケース: `verify.py` と同じ方針の乱数ケース（シード固定、3,000 件）、§14 の 1 の敵対ケース、上流テストと README 例の入力。ファイルは 4 MiB 以下（超えたら生成を止める）。
- Rust の `tests/compat.rs`: `from_parts`（ctor の値から作る）で `AsearchTsLib` == `lib`、`AsearchTs` == `src` を全件。`new`（Rust 単体の入口）でも同じ比較をする。mode は asearch.ts と同じく「JSON の 1 → Include、2 → WildcardSpace、それ以外（3、"1"、null、undefined、NaN）→ Exact」に写す。pattern に孤立サロゲートを含むケースは `&str` で表せないので `new` の比較から除く（`from_parts` では検査する）。それ以外で食い違ったケースは DECISIONS に列挙する（ロケール依存の文字以外では一致する想定）。

<a id="section-19-7"></a>

### 19.7 JS のテスト（`node:test`）

1. フィクスチャの再生（両プロファイル、JS API 経由）。
2. 境界の差分テスト: 同梱の上流 lib を同じプロセスで動かし、mode（`0, 1, 2, 3, '1', null, NaN, 'Include', undefined, 1.0`）、ambig（`0, 1, '1', [1], [0], null, true, 1.5, -1, -0, NaN, Infinity, 1e21, '01', ' 1', 31, 32, 33, 65535`、`valueOf`／`toString` を持つオブジェクト、`valueOf` が例外を投げるオブジェクト、BigInt、Symbol）、String オブジェクトの pattern／query について、結果と例外の型が一致すること。逸脱（D10: 非文字列、65,535 超、`'length'`、プロパティ書き換え）は別テストで「仕様どおりの逸脱」として検査する。上流 lib を、数値で 1e7 以上かつ 2^32 − 1 未満の ambig で呼ばない（1e7 で約 1 秒、4294967294 では Node のプロセスが致命的なメモリ不足で落ちた。2^32 − 1 以上の数値はすぐ RangeError になるので呼んでよい。文字列の `'4294967294'` は `+ 1` が連結になるので安全）。
3. 形のテスト: `Object.keys(MatchMode)`、`Asearch.MAXLENGTH`、インスタンスの自前プロパティの名前・順・値、`masks` のキー順、長さ超過の例外が `Error.prototype` の直接のインスタンスでメッセージが一致すること。
4. 上流テストの移植: `vendor/upstream/asearch.ts/src/index.test.ts` の 36 個のアサーションを、値を変えずに `node:test` へ機械的に移す（`t.is(a, b)` → `assert.strictEqual(a, b)`、`t.true(x)` → `assert.strictEqual(x, true)`、`t.false(x)` → `assert.strictEqual(x, false)`）。`test.before` の `process.env.ASEARCH_VERBOSE = '1'` は移さない（verbose を実装しないため。D10）。
5. README 例: `README.md` と `README-ja.md` のそれぞれ 19〜31 行目と 94〜100 行目のコード例を、コメントの値をアサーションにして通す（同梱の lib で実行した値とコメントが一致することは確認済み、2026-10-20）。両ファイルの 80〜86 行目は node-asearch の例なので対象外。
6. d.ts の互換テスト（§19.5）。
7. §19.5.1 の初期化状態テスト。JSラッパのtypecheckとformat/lint、npm packしたtarballだけを別ディレクトリへ展開してNode ESM/CJSとweb importが動く配布検査もM0に含める。

<a id="section-19-8"></a>

### 19.8 ブラウザのスモークテスト（`tests/browser/`、Chromium だけ）

- 設定は `tests/browser/playwright.config.mts`。`webServer` で**リポジトリのルート**を配信する小さな静的サーバ（Node の `http` で書く。`file://` では .wasm を fetch できない）を立て、`/tests/browser/fixtures/index.html` から `/packages/asearch-wasm/dist/web.mjs` を import し、`/crates/asearch-rs/tests/fixtures/compat.json` を fetch する。実行前に `dist/` をビルドしておく（CI では wasm-js ジョブの成果物を使うか、同じジョブでビルドする）。
- module Workerにも同じweb entryをimportし、代表ケースを照合する（DOM APIに依存しないこと）。
- ページ内で `typeof process === 'undefined'` を確かめ、README 例と、フィクスチャから選んだ 20 件（非 BMP と WildcardSpace を含む）を照合する。期待値は既定プロファイルでは `src` 列、`profile: 'asearch.ts-lib'` では `lib` 列。上流テスト（`process.env` を使う）は流用しない。

<a id="section-19-9"></a>

### 19.9 CI（`.github/workflows/ci.yml`、GitHub Actions）

- `rust`: `cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`。
- `wasm-js`（Node 22 と 24 の matrix、JSラッパのtypecheckとformat/lint、packした配布物のテストも含む）: `rustup target add wasm32-unknown-unknown`、`cargo install wasm-bindgen-cli --version 0.2.129 --locked`、`pnpm install --frozen-lockfile`、`pnpm build`、`pnpm test`。
- Chromium smokeはwasm-jsのNode 24ジョブ内で`pnpm exec playwright install --with-deps chromium`後に`pnpm test:browser`を実行し、重複ビルドを避ける（D19）。
- すべてのジョブで `LANG=en_US.UTF-8`、`LC_ALL=en_US.UTF-8`。フィクスチャの再生成は CI で行わず、`gen.mjs --check`（生成し直してコミット済みのファイルとバイト一致を確かめる）を wasm-js ジョブの Node 22 でだけ走らせる。
- push 先が無い環境では CI の緑を確かめられない。その場合は同じコマンドを手元で通し、報告に「CI は未実行」と書く。

<a id="section-19-10"></a>

### 19.10 ライセンス表示

- 移植元の出典とライセンスの宣言を `NOTICE` に置く（`vendor/upstream/README.md` の表）: teramotodaiki/asearch.ts（package.json の宣言は ISC、author は teramotodaiki。LICENSE ファイルと著作権行は無い）、shokai/node-asearch（BSD-2-Clause、author は Sho Hashimoto。同上）、masui/asearch-ruby（README に MIT の全文、著作権者欄は "FIXME full name"。ソースの先頭は "Copyright 2011 Pitecan Systems. All rights reserved." で README と食い違う。本リポジトリは asearch-ruby のコードを取り込まず、アルゴリズムの出典として記すだけ）。上流に無い年や著作権者を推測で書き足さない。各項目には、名前、URL、版またはコミット、宣言されたライセンス、package.json の author、そのライセンスの許諾文（`vendor/upstream/licenses/` の ISC.txt・BSD-2-Clause.txt・MIT.txt から本文を写し、著作権行は「上流に著作権行は無い（package.json の author: …）」と書く）を載せる。`vendor/upstream/` に同梱した上流のファイルは、それぞれ元のライセンスのままであることも書く。
- プロジェクト自体のライセンスは `MIT OR Apache-2.0`（D7）。`LICENSE-APACHE` は `vendor/upstream/licenses/LICENSE-APACHE` をそのまま使う。`LICENSE-MIT` は `vendor/upstream/licenses/MIT.txt` を雛形にし、著作権表記はルートの `LICENSE-MIT` を正本とし、追加・改変部分だけへ適用する。Cargo.toml は `license = "MIT OR Apache-2.0"`、package.json は `"license": "(MIT OR Apache-2.0)"`。

<a id="section-19-11"></a>

### 19.11 その他

- M0 のコミットは 1 つ（M1以降は変更単位で分ける）。報告書式のベンチ表は「M1 で計測」と書けばよい。
- §0の「1 text は 65,535 code unit 以下」は索引（M2 以降）の制限で、互換 API には適用しない。
- `ScalarWm` の語幅ジェネリック化（u64／u128／`MultiWord<N>`）、`matchLevel`、`'ideal'` プロファイル、明示 `free()`／`[Symbol.dispose]` は M1。

---
