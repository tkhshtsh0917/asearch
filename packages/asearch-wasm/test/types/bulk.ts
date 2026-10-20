import {
  AsearchIndex,
  buildDelta,
  buildIndex,
  byLength,
  type Doc,
  fixed,
  type SearchResult,
} from "../../dist/index.js";

const docs: Doc[] = [{ key: 1, texts: ["質問です"] }];
using index = AsearchIndex.fromDocs(docs, {
  normalizer: "ja-basic-v1",
  limits: { maxPatternUnits: 128 },
});
const result: SearchResult = index.search("質問", {
  maxAmbig: byLength({ every: 4, max: 3 }),
  countTotal: true,
});
const bytes: Uint8Array = buildIndex(docs);
AsearchIndex.fromBytes(bytes)
  .search("質問", { maxAmbig: fixed(1) })
  .copy();
AsearchIndex.fromStream(new ReadableStream<Uint8Array>(), {
  maxBytes: 1024,
}).then((index) => index.free());
// @ts-expect-error Integrity descriptors require every content field.
AsearchIndex.fromBytes(bytes, { expected: {} });
const ids: Uint32Array = result.ids;
const levels: Uint8Array = result.levels;
void ids;
void levels;

const session = index.session({ maxCacheBytes: 1024 * 1024, countTotal: true });
session.update("検索").copy();
session.reset();
session.free();

index.search("ｶﾞ", {
  maxAmbig: ({ normalized, mode, profile }) =>
    normalized.length > 5 && mode === 2 && profile === "ideal" ? 1 : 0,
});

import { diffManifest, type Manifest } from "../../dist/index.js";

declare const current: Manifest | undefined;
declare const next: Manifest;
const change = diffManifest(current, next);
if (change.kind === "append") change.add.map((segment) => segment.integrity);

const delta: Uint8Array = buildDelta(docs, [1]);
void delta;

import { type MergePart, mergeResults } from "../../dist/index.js";

const workerPart: MergePart = {
  ids: result.ids,
  levels: result.levels,
  refs: result.refs,
  orders: result.orders,
  length: result.length,
};
mergeResults([workerPart, result], 30).parts;

import {
  type IntentEntry,
  IntentIndex,
  type IntentSearchOptions,
} from "../../dist/index.js";

const entries: IntentEntry[] = [{ key: 1, expr: "(ライト|電球)を交換したい" }];
using intent = IntentIndex.fromEntries(entries);
const intentOptions: IntentSearchOptions = { engine: "dag", countTotal: true };
const intentResult: SearchResult = intent.search("電球 交換", intentOptions);
const explanation = intent.explain(
  intentResult.refs[0],
  "電球 交換",
  intentOptions,
);
void explanation;
// @ts-expect-error IntentIndex does not expose blocked corpus search.
intent.search("a", { engine: "transposed-blocked" });
