import assert from "node:assert/strict";
import { createRequire } from "node:module";
import test from "node:test";
import * as esm from "../dist/node.mjs";

const cjs = createRequire(import.meta.url)("../dist/node.cjs");
for (const [name, api] of [
  ["ESM", esm],
  ["CJS", cjs],
]) {
  test(`${name}: explain returns original UTF-16 ranges with deterministic alignment`, () => {
    const index = api.AsearchIndex.fromStrings([
      "ｶﾞｸ",
      "a x a b",
      "😀a",
      "abxxab",
    ]);
    try {
      for (const [pattern, key, options, spans] of [
        ["が", 0, { mode: 1 }, [[0, 2]]],
        [
          "a b",
          1,
          {},
          [
            [4, 5],
            [6, 7],
          ],
        ],
        ["a", 2, { mode: 1 }, [[2, 3]]],
        ["ab", 3, { mode: 1 }, [[0, 2]]],
      ]) {
        const found = index.search(pattern, { ...options, maxAmbig: 0 }),
          i = [...found.ids].indexOf(key);
        assert.ok(i >= 0);
        const explanation = index.explain(found.refs[i], pattern, {
          ...options,
          maxAmbig: 0,
        });
        assert.equal(explanation.level, found.levels[i]);
        assert.equal(explanation.coordinateSpace, "original-utf16");
        assert.deepEqual(explanation.spans, spans);
      }
      const ref = index.search("が", { maxAmbig: 0 }).refs[0];
      assert.equal(index.explain(ref, "different", { maxAmbig: 0 }), null);
      assert.deepEqual(
        index.explain(ref, "が", { original: "ガク", maxAmbig: 0 }).spans,
        [[0, 1]],
      );
      assert.throws(() => index.explain(ref, "が", { original: "other" }), {
        code: "INVALID_INPUT",
      });
      assert.throws(() => index.explain(0, "が"), { code: "INVALID_INPUT" });
    } finally {
      index.free();
    }
  });
  test(`${name}: refs survive failures and expire after adoption; other roots are rejected`, async () => {
    const index = api.AsearchIndex.fromStrings(["old"]),
      other = api.AsearchIndex.fromStrings(["other"]),
      session = index.session();
    try {
      const copied = session.update("old").copy(),
        ref = copied.refs[0];
      assert.throws(() => other.explain(ref, "old"), { code: "INVALID_INPUT" });
      await assert.rejects(index.addSegment(new Uint8Array(64)));
      assert.equal(index.explain(ref, "old").text, "old");
      await index.addSegment(api.buildDelta([{ key: 0, texts: ["new"] }], [0]));
      assert.equal(copied.refs[0], ref);
      assert.throws(() => index.explain(ref, "old"), { code: "INVALID_INPUT" });
      const next = session.update("new").refs[0];
      assert.equal(index.explain(next, "new").text, "new");
      assert.notEqual(next, ref);
    } finally {
      session.free();
      other.free();
      index.free();
    }
  });
  test(`${name}: explain applies custom policies and host casing and rechecks disposal`, () => {
    const index = api.AsearchIndex.fromStrings(["I", "ı"], {
      normalizer: "none",
    });
    const lower = String.prototype.toLocaleLowerCase;
    try {
      String.prototype.toLocaleLowerCase = function () {
        return lower.call(this, "tr");
      };
      const found = index.search("I", { mode: 0, maxAmbig: 0 });
      let calls = 0;
      assert.equal(
        index.explain(found.refs[1], "I", {
          mode: 0,
          maxAmbig: () => {
            calls++;
            return 0;
          },
        }).level,
        0,
      );
      assert.equal(calls, 1);
      String.prototype.toLocaleLowerCase = function () {
        index.free();
        return lower.call(this);
      };
      assert.throws(() => index.explain(found.refs[0], "I"), {
        code: "DISPOSED",
      });
    } finally {
      String.prototype.toLocaleLowerCase = lower;
      index.free();
    }
  });
}

test("explain rejects an oversized alignment matrix without changing search data", () => {
  const index = esm.AsearchIndex.fromStrings(["a".repeat(1000)], {
    limits: { maxPatternUnits: 64, maxWorkingBytes: 200000 },
  });
  try {
    const pattern = "a".repeat(64),
      found = index.search(pattern, { mode: 1, maxAmbig: 0 });
    assert.equal(found.length, 1);
    assert.throws(
      () => index.explain(found.refs[0], pattern, { mode: 1, maxAmbig: 0 }),
      { code: "RESOURCE_LIMIT" },
    );
    assert.deepEqual(
      index.search(pattern, { mode: 1, maxAmbig: 0 }).ids,
      found.ids,
    );
  } finally {
    index.free();
  }
});
