import assert from "node:assert/strict";
import { createRequire } from "node:module";
import test from "node:test";
import * as esm from "../dist/node.mjs";

for (const [name, api] of [
  ["ESM", esm],
  ["CJS", createRequire(import.meta.url)("../dist/node.cjs")],
]) {
  test(`${name}: owned results outlive APIs and disposal but refs expire on adoption`, async () => {
    const index = api.AsearchIndex.fromStrings(["old"]);
    const result = index.search("old");
    const copy = result.copy();
    const saved = structuredClone({
      ids: result.ids,
      refs: result.refs,
      levels: result.levels,
      orders: result.orders,
    });
    index.search("other");
    await index.addSegment(api.buildDelta([{ key: 0, texts: ["new"] }], [0]));
    assert.throws(() => index.explain(result.refs[0], "new"), {
      code: "INVALID_INPUT",
    });
    index.free();
    for (const key of ["ids", "refs", "levels", "orders"])
      assert.deepEqual(result[key], saved[key]);
    result.ids[0] = 99;
    result.stats.blocks = 999;
    assert.deepEqual(copy.ids, saved.ids);
    assert.notEqual(copy.stats.blocks, 999);
  });
}
