import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createRequire } from "node:module";
import test from "node:test";
import * as esm from "../dist/node.mjs";

const cjs = createRequire(import.meta.url)("../dist/node.cjs");
const descriptor = (bytes) => ({
  id: new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
    .getBigUint64(24, true)
    .toString(16)
    .padStart(16, "0"),
  url: "base.asix",
  bytes: bytes.length,
  texts: 2,
  integrity: `sha384-${createHash("sha384").update(bytes).digest("base64")}`,
});
function stream(bytes) {
  let offset = 0;
  return new ReadableStream({
    pull(controller) {
      if (offset === bytes.length) controller.close();
      else {
        controller.enqueue(bytes.subarray(offset, offset + 17));
        offset = Math.min(offset + 17, bytes.length);
      }
    },
  });
}
for (const [name, api] of [
  ["ESM", esm],
  ["CJS", cjs],
]) {
  test(`${name}: native SHA-384 loader agrees with Node crypto for bytes and streams`, async () => {
    const bytes = api.buildIndex([{ key: 1, texts: ["ｶﾞと検索", "別の質問"] }]);
    const expected = descriptor(bytes);
    const loaded = api.AsearchIndex.fromBytes(bytes, { expected });
    const streamed = await api.AsearchIndex.fromStream(stream(bytes), {
      expected,
    });
    try {
      assert.deepEqual([...loaded.search("検索", { maxAmbig: 0 }).ids], [1]);
      assert.deepEqual(streamed.toBytes(), bytes);
    } finally {
      loaded.free();
      streamed.free();
    }
    for (const change of [
      { id: "0000000000000000" },
      { bytes: bytes.length + 1 },
      { texts: 3 },
      { integrity: `sha384-${"A".repeat(64)}` },
    ]) {
      const bad = { ...expected, ...change };
      assert.throws(
        () => api.AsearchIndex.fromBytes(bytes, { expected: bad }),
        { code: "INVALID_INDEX" },
      );
      const body = stream(bytes);
      await assert.rejects(
        api.AsearchIndex.fromStream(body, { expected: bad }),
        { code: "INVALID_INDEX" },
      );
      assert.equal(body.locked, false);
    }
    const altered = bytes.slice();
    altered[altered.length - 1] ^= 1;
    assert.throws(() => api.AsearchIndex.fromBytes(altered, { expected }), {
      code: "INVALID_INDEX",
    });
    const valid = api.AsearchIndex.fromBytes(bytes, { expected });
    valid.free();
  });
}
