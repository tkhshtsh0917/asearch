import assert from "node:assert/strict";
import { createRequire } from "node:module";
import test from "node:test";
import * as esm from "../dist/node.mjs";

const require = createRequire(import.meta.url);
const cjs = require("../dist/node.cjs");
const memory = require("../dist/node/asearch_wasm.js").index_memory();

function stream(bytes, size) {
  let offset = 0;
  return new ReadableStream({
    pull(controller) {
      if (offset === bytes.length) {
        controller.close();
        return;
      }
      controller.enqueue(
        bytes.subarray(offset, Math.min(bytes.length, offset + size)),
      );
      offset = Math.min(bytes.length, offset + size);
    },
  });
}

for (const [name, api] of [
  ["ESM", esm],
  ["CJS", cjs],
]) {
  test(`${name}: stream boundaries and memory growth retain the same index`, async () => {
    const bytes = api.buildIndex([
      { key: 2, texts: ["ｶﾞの質問😀"] },
      { key: 1, texts: ["ABC", ""] },
    ]);
    for (const size of [1, 3, 63, 64, 65, bytes.length]) {
      const body = stream(bytes, size);
      const index = await api.AsearchIndex.fromStream(body);
      try {
        assert.deepEqual(index.toBytes(), bytes);
        assert.deepEqual(
          [...index.search("がの質問😀", { maxAmbig: 0 }).ids],
          [2],
        );
      } finally {
        index.free();
      }
      assert.equal(body.locked, false);
    }
    let step = 0;
    const body = new ReadableStream({
      pull(controller) {
        if (step === 0) controller.enqueue(bytes.subarray(0, 64));
        else if (step === 1) {
          memory.grow(1);
          controller.enqueue(bytes.subarray(64));
        } else controller.close();
        step++;
      },
    });
    const loaded = await api.AsearchIndex.fromStream(body);
    try {
      assert.deepEqual(loaded.toBytes(), bytes);
    } finally {
      loaded.free();
    }
  });

  test(`${name}: malformed, truncated, overflowing and failed streams clean up`, async () => {
    const valid = api.buildIndex([{ key: 1, texts: ["hello"] }]);
    const invalid = valid.slice();
    invalid[0] = 0;
    const corrupt = valid.slice();
    corrupt[corrupt.length - 1] ^= 1;
    for (const bytes of [
      valid.subarray(0, 63),
      valid.subarray(0, valid.length - 1),
      new Uint8Array([...valid, 1]),
      invalid,
      corrupt,
    ]) {
      const body = stream(bytes, 65);
      await assert.rejects(api.AsearchIndex.fromStream(body), {
        code: "INVALID_INDEX",
      });
      assert.equal(body.locked, false);
    }
    let cancelled = false;
    const large = valid.slice(0, 64);
    new DataView(large.buffer).setBigUint64(16, 2n ** 53n, true);
    const body = new ReadableStream({
      start(controller) {
        controller.enqueue(large);
      },
      cancel() {
        cancelled = true;
      },
    });
    await assert.rejects(api.AsearchIndex.fromStream(body), {
      code: "RESOURCE_LIMIT",
    });
    assert.equal(cancelled, true);
    assert.equal(body.locked, false);
    const ioError = new Error("read failed");
    let reads = 0;
    const failed = new ReadableStream({
      pull(controller) {
        if (reads++ === 0) controller.enqueue(valid.subarray(0, 64));
        else controller.error(ioError);
      },
    });
    await assert.rejects(
      api.AsearchIndex.fromStream(failed),
      (error) => error === ioError,
    );
    assert.equal(failed.locked, false);
    await assert.rejects(
      api.AsearchIndex.fromStream(stream(valid, 64), { maxBytes: 64 }),
      { code: "RESOURCE_LIMIT" },
    );
    assert.throws(() => api.AsearchIndex.fromBytes(valid, { maxBytes: 64 }), {
      code: "RESOURCE_LIMIT",
    });
    // A finish-time checksum error consumed and released its Rust buffer.
    const after = await api.AsearchIndex.fromStream(stream(valid, 64));
    after.free();
  });
}

test("a never-settling cancellation cannot pin native staging or the stream lock", async () => {
  let cancelled = false;
  const body = new ReadableStream({
    start(controller) {
      controller.enqueue(new Uint8Array(64));
    },
    cancel() {
      cancelled = true;
      return new Promise(() => {});
    },
  });
  await assert.rejects(esm.AsearchIndex.fromStream(body), {
    code: "INVALID_INDEX",
  });
  assert.equal(cancelled, true);
  assert.equal(body.locked, false);
});

test("stream subclasses cannot substitute an unbranded reader and skip native cleanup", async () => {
  let calls = 0;
  class Body extends ReadableStream {
    getReader() {
      calls++;
      throw new Error("custom reader must not be called");
    }
  }
  const bytes = esm.buildIndex([{ key: 1, texts: ["first"] }]);
  const body = new Body({
    start(controller) {
      controller.enqueue(bytes);
      controller.close();
    },
  });
  const index = await esm.AsearchIndex.fromStream(body);
  try {
    const delta = esm.buildDelta([{ key: 1, texts: ["next"] }], [1]);
    await index.addSegment(
      new Body({
        start(controller) {
          controller.enqueue(delta);
          controller.close();
        },
      }),
    );
    assert.deepEqual([...index.search("next").ids], [1]);
    assert.equal(calls, 0);
    assert.equal(body.locked, false);
  } finally {
    index.free();
  }
  await assert.rejects(
    esm.AsearchIndex.fromStream({
      getReader() {
        throw new Error("fake");
      },
    }),
    TypeError,
  );
});
