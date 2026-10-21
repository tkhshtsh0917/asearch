import assert from "node:assert/strict";
import { createRequire } from "node:module";
import test from "node:test";
import * as esm from "../dist/node.mjs";

const cjs = createRequire(import.meta.url)("../dist/node.cjs");
function unexpected() {
  throw new Error("Caller-owned TypedArray hooks must not run");
}
class HostileBytes extends Uint8Array {
  get length() {
    return unexpected();
  }
  get byteLength() {
    return unexpected();
  }
  get byteOffset() {
    return unexpected();
  }
  get buffer() {
    return unexpected();
  }
  static get [Symbol.species]() {
    return unexpected();
  }
  subarray() {
    return unexpected();
  }
  slice() {
    return unexpected();
  }
  [Symbol.iterator]() {
    return unexpected();
  }
}
function hostile(bytes, subclass) {
  // Nonzero offsets catch accidental reads of the complete backing buffer.
  const buffer = new ArrayBuffer(bytes.length + 12);
  const value = subclass
    ? new HostileBytes(buffer, 7, bytes.length)
    : new Uint8Array(buffer, 7, bytes.length);
  Uint8Array.prototype.set.call(value, bytes);
  if (!subclass) {
    for (const key of [
      "length",
      "byteLength",
      "byteOffset",
      "buffer",
      "constructor",
    ])
      Object.defineProperty(value, key, { get: unexpected });
    for (const key of ["subarray", "slice", Symbol.iterator])
      Object.defineProperty(value, key, { value: unexpected });
  }
  return value;
}
function stream(...values) {
  return new ReadableStream({
    start(controller) {
      for (const value of values) controller.enqueue(value);
      controller.close();
    },
  });
}
for (const [name, api] of [
  ["ESM", esm],
  ["CJS", cjs],
]) {
  for (const subclass of [false, true]) {
    test(`${name}: ${subclass ? "subclass" : "own getters"} cannot intercept byte loading or updates`, async () => {
      const base = api.buildIndex([{ key: 0, texts: ["old"] }]);
      const delta = api.buildDelta([{ key: 0, texts: ["new"] }], [0]);
      const index = api.AsearchIndex.fromBytes(hostile(base, subclass));
      try {
        assert.deepEqual(index.toBytes(), base);
        for (const source of [
          hostile(delta, subclass),
          stream(hostile(delta, subclass)),
        ]) {
          await index.replaceSegments([{ source: hostile(base, subclass) }]);
          await index.addSegment(source);
          assert.deepEqual([...index.search("new").ids], [0]);
        }
        await index.replaceSegments([{ source: hostile(base, subclass) }]);
        const before = index.toBytes();
        const ref = index.search("old").refs[0];
        const extra = new Uint8Array(delta.length + 1);
        extra.set(delta);
        for (const source of [
          hostile(extra, subclass),
          stream(hostile(extra, subclass)),
        ]) {
          await assert.rejects(index.addSegment(source), {
            code: "INVALID_INDEX",
          });
          assert.deepEqual(index.toBytes(), before);
          assert.equal(index.search("old").refs[0], ref);
        }
        // An oversized late chunk must be rejected against the remaining body.
        await assert.rejects(
          index.addSegment(
            stream(
              hostile(delta.subarray(0, 64), subclass),
              hostile(new Uint8Array(delta.length), subclass),
            ),
          ),
          { code: "INVALID_INDEX" },
        );
        assert.deepEqual(index.toBytes(), before);
        await index.addSegment(hostile(delta, subclass));
      } finally {
        index.free();
      }
      for (const body of [
        stream(hostile(base, subclass)),
        stream(
          hostile(base.subarray(0, 31), subclass),
          hostile(base.subarray(31), subclass),
        ),
      ]) {
        const loaded = await api.AsearchIndex.fromStream(body);
        assert.deepEqual(loaded.toBytes(), base);
        loaded.free();
      }
      const extra = new Uint8Array(base.length + 1);
      extra.set(base);
      await assert.rejects(
        api.AsearchIndex.fromStream(stream(hostile(extra, subclass))),
        { code: "INVALID_INDEX" },
      );
    });
  }
  test(`${name}: byte loader rejects proxies and detached buffers safely`, () => {
    const bytes = api.buildIndex([{ key: 0, texts: ["ok"] }]);
    assert.throws(
      () => api.AsearchIndex.fromBytes(new Proxy(bytes, {})),
      TypeError,
    );
    const detached = bytes.slice();
    structuredClone(detached.buffer, { transfer: [detached.buffer] });
    assert.throws(() => api.AsearchIndex.fromBytes(detached), TypeError);
    assert.throws(
      () => api.AsearchIndex.fromBytes(new Uint16Array(32)),
      TypeError,
    );
  });
}
