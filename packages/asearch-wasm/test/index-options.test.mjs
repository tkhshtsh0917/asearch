import assert from "node:assert/strict";
import test from "node:test";
import { AsearchIndex, byLength } from "../dist/node.mjs";

test("explicit null and object coercions cannot silently select defaults", () => {
  const index = AsearchIndex.fromStrings(["a"]);
  try {
    for (const options of [
      { mode: null },
      { wrap: null },
      { maxAmbig: null },
      { limit: null },
      { countTotal: null },
      { engine: null },
    ]) {
      assert.throws(() => index.search("a", options));
      assert.deepEqual([...index.search("a", { maxAmbig: 0 }).ids], [0]);
    }
  } finally {
    index.free();
  }
  for (const options of [
    { profile: null },
    { normalizer: null },
    { blockWords: null },
    { limits: null },
    { limits: { maxTexts: null } },
    { limits: { maxSegments: null } },
  ])
    assert.throws(() => AsearchIndex.fromStrings(["a"], options));
  for (const options of [
    { every: null },
    { max: null },
    { unit: null },
    { includeWildcards: null },
  ])
    assert.throws(() => byLength(options));
  assert.throws(() =>
    AsearchIndex.fromDocs([{ key: 1, order: null, texts: ["a"] }]),
  );
  assert.throws(
    () =>
      AsearchIndex.fromStrings(["a"], {
        profile: {
          toString() {
            throw new Error("must not coerce");
          },
        },
      }),
    { code: "INVALID_INPUT" },
  );
});

test("unknown non-enumerable and symbol options are rejected", () => {
  const options = Object.defineProperty({}, "unexpected", { value: {} });
  assert.throws(() => AsearchIndex.fromBytes(new Uint8Array(64), options), {
    code: "INVALID_INPUT",
  });
  assert.throws(
    () => AsearchIndex.fromStrings(["a"], { [Symbol("unexpected")]: true }),
    { code: "INVALID_INPUT" },
  );
  assert.throws(() => AsearchIndex.fromStrings(["a"], []), TypeError);
});

test("text arrays use bounded indexed snapshots rather than custom iterators", () => {
  const texts = ["a"];
  texts[Symbol.iterator] = function* () {
    yield "unexpected";
  };
  const index = AsearchIndex.fromDocs([{ key: 1, texts }]);
  try {
    assert.deepEqual([...index.search("a", { maxAmbig: 0 }).ids], [1]);
  } finally {
    index.free();
  }
  assert.throws(
    () =>
      AsearchIndex.fromDocs([{ key: 1, texts: ["", "", ""] }], {
        limits: { maxTexts: 2 },
      }),
    { code: "RESOURCE_LIMIT" },
  );
});

test("policy descriptors require fields and snapshot their discriminant once", () => {
  const index = AsearchIndex.fromStrings(["abcd"]);
  try {
    for (const maxAmbig of [
      { kind: "by-length" },
      { kind: "by-length", every: 4 },
      { kind: "by-length", max: 3 },
    ])
      assert.throws(() => index.search("abcd", { maxAmbig }));
    let reads = 0;
    const maxAmbig = {
      get kind() {
        reads++;
        return "by-length";
      },
      every: 4,
      max: 3,
    };
    assert.deepEqual([...index.search("abcd", { maxAmbig }).ids], [0]);
    assert.equal(reads, 1);
  } finally {
    index.free();
  }
});
