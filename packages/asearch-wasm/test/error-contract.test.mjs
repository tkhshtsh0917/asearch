import assert from "node:assert/strict";
import { createRequire } from "node:module";
import test from "node:test";
import * as esm from "../dist/node.mjs";

for (const [name, api] of [
  ["ESM", esm],
  ["CJS", createRequire(import.meta.url)("../dist/node.cjs")],
]) {
  test(`${name}: bulk type, value and format failures have stable codes`, async () => {
    const index = api.AsearchIndex.fromStrings(["ok"]);
    assert.throws(() => api.buildIndex(123), {
      code: "INVALID_INPUT",
      name: "TypeError",
    });
    assert.throws(() => api.buildIndex([null]), {
      code: "INVALID_INPUT",
      name: "TypeError",
    });
    try {
      for (const run of [
        () => index.search("ok", { limit: "30" }),
        () => index.search(1),
        () => api.byLength({ every: "4" }),
        () => api.AsearchIndex.fromBytes(new Uint16Array(64)),
        () => api.IntentIndex.fromEntries(null),
      ]) {
        assert.throws(run, { name: "TypeError", code: "INVALID_INPUT" });
      }
      assert.throws(() => index.search("ok", { limit: -1 }), {
        code: "INVALID_INPUT",
      });
      assert.throws(
        () =>
          api.AsearchIndex.fromStrings(["ok"], {
            normalizer: { id: "ja-basic", version: 2, options: 31 },
          }),
        { code: "UNSUPPORTED_FORMAT" },
      );
      await assert.rejects(index.replaceSegments(null), {
        name: "TypeError",
        code: "INVALID_INPUT",
      });
      await assert.rejects(
        api.AsearchIndex.fromStream(
          new ReadableStream({
            start(c) {
              c.enqueue(1);
              c.close();
            },
          }),
        ),
        { name: "TypeError", code: "INVALID_INPUT" },
      );
      await assert.rejects(api.AsearchIndex.fromStream({}), {
        code: "INVALID_INPUT",
      });
      assert.deepEqual([...index.search("ok").ids], [0]);
    } finally {
      index.free();
    }
    assert.throws(() => index.search("ok"), { code: "DISPOSED" });
    assert.throws(
      () => new api.Asearch(123),
      (e) => e instanceof TypeError && e.code === undefined,
    );
  });
}
