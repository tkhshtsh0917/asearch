# asearch-wasm

一括検索には`AsearchIndex.fromDocs/fromStrings/fromBytes`、索引生成には`buildIndex`を使えます。既定はIdeal/code pointで、asearch.ts/asearch.ts-libも選べます。正規化後の大小文字候補はJSのホストlocaleで生成します。保存/ロードは現在ASIX base専用です。`search(pattern, { maxAmbig: fixed(1), countTotal: true })`はkey順ではなく(level,order,key)順のids・levels・refs・ordersと、必要ならtotalsを返します。結果配列は現在JS所有のコピーです。詳細・コード例・未実装の範囲は[プロジェクトREADME](https://github.com/tkhshtsh0917/asearch#使い方)を参照してください。

Rust/WebAssembly port of asearch.ts 1.0.3. This version implements matching and compatibility profiles; bulk indexes, normalization and bounded incremental sessions are available; server-segment updates are not provided by this version.

Node ESM and CommonJS initialize synchronously:

```js
import { Asearch, MatchMode } from "asearch-wasm";
const pattern = new Asearch("猫である", MatchMode.Include);
console.log(pattern.match("吾輩は猫である。")); // true
```

For a browser, serve the package's complete `dist/` directory over HTTP(S), including `web/asearch_wasm.js` and `web/asearch_wasm_bg.wasm`. Import `dist/web.mjs`. Its top-level await attempts initialization automatically. It does not reference `process` or the DOM and can also run in a module Worker. CSP must allow the module, its WASM fetch and WebAssembly compilation as appropriate for the host.

If automatic initialization fails, importing still succeeds but `ready` rejects. Retry with `await init(new URL("/assets/asearch_wasm_bg.wasm", location.href))`. `ready` is a live binding for the current attempt; a previously saved Promise remains unchanged. Concurrent `init()` calls without inputs share a Promise. Explicit input during initialization, or after success, is rejected. The Node `init()` function is a no-op. Missing/corrupt local WASM makes Node imports fail. Valid WASM with incompatible exports also fails initialization before its exports are cached, allowing the browser to retry with the correct artifact.

`new Asearch(pattern, mode?, {profile?})` supports Exact (0), Include (1), WildcardSpace (2). Maximum pattern length is 31 UTF-16 units, including spaces. The default `asearch.ts` profile iterates text by code point; `asearch.ts-lib` reproduces the published library's UTF-16 behavior, including its non-BMP mismatch. `match(query, ambig=0)` returns a boolean at that level; results are not monotonic in ambig. The compatibility path accepts levels through 65,535, but large levels are deliberately expensive and not a performance target.

Strings and String objects are accepted; other pattern/text inputs throw TypeError. Levels above 65,535 throw RangeError. The upstream accidental `ambig='length'` behavior is not reproduced. Constructor properties retain upstream names and values, but mutation after construction does not recompile the matcher. GC reclaims handles via FinalizationRegistry without an immediate-release guarantee. The API provides idempotent `free()` and `[Symbol.dispose]()`. Matching after release throws an Error with code `DISPOSED` before converting inputs. On hosts without native Symbol.dispose, use free(); the symbol fallback is Symbol.for("Symbol.dispose"). Include `ESNext.Disposable` in TypeScript lib settings. 

The API provides `{profile: "ideal"}`: code-point iteration with monotonic Levenshtein initialization. For example, `new Asearch("a", 0, {profile: "ideal"}).match("", 2)` is true, while the compatibility profiles return false. Ideal does not wrap shifts above level 31. It retains the same casing, modes, argument conversion, disposal and 31 UTF-16 pattern limit. It does not normalize input. Longer patterns are currently available through Rust's separate `ScalarWm<W>` API.

New contributions: MIT OR Apache-2.0. Upstream-derived material retains its own terms. Distribute LICENSE-MIT, LICENSE-APACHE, NOTICE and the included notices with the code/WASM.

The API also provides `matchLevel(query, maxAmbig = 3)`: the first accepted level or -1. It evaluates all levels in one Rust scan. maxAmbig must be a primitive number, an integer in 0..31; other types throw TypeError and invalid numeric values throw RangeError. It preserves the selected profile and rejects disposed handles. This bounded API does not use the legacy shift-wrapping path above 31.

`index.session(options)` shares the immutable corpus with an independent query/workspace. `session.update(pattern)` matches full search through normalized edits and K changes; `reset()` clears checkpoints and `free()` releases its reservation. The default cache ceiling is 32 MiB, counted in the parent index budget. Set `maxCacheBytes: 0` to use blocked search. Freeing the parent invalidates all child sessions.
