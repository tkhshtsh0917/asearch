import { expect, test } from "@playwright/test";

test("byte loaders ignore own getters and subclass hooks before WASM transport", async ({
  page,
}) => {
  await page.goto("/tests/browser/fixtures/index.html");
  const result = await page.evaluate(async () => {
    // @ts-expect-error Browser-only absolute module URL.
    const api = await import("/packages/asearch-wasm/dist/web.mjs");
    function unexpected(): never {
      throw new Error("Caller-owned byte hooks must not run");
    }
    class Bytes extends Uint8Array {
      subarray(): never {
        return unexpected();
      }
      static get [Symbol.species](): never {
        return unexpected();
      }
    }
    function hostile(source: Uint8Array, subclass: boolean): Uint8Array {
      const value = subclass ? new Bytes(source) : new Uint8Array(source);
      for (const key of ["length", "byteLength", "byteOffset", "buffer"])
        Object.defineProperty(value, key, { get: unexpected });
      return value;
    }
    function stream(value: Uint8Array): ReadableStream<Uint8Array> {
      return new ReadableStream({
        start(controller) {
          controller.enqueue(value);
          controller.close();
        },
      });
    }
    const base = api.buildIndex([{ key: 0, texts: ["old"] }]);
    const delta = api.buildDelta([{ key: 0, texts: ["new"] }], [0]);
    const ids = [];
    for (const subclass of [false, true]) {
      const index = api.AsearchIndex.fromBytes(hostile(base, subclass));
      const loaded = await api.AsearchIndex.fromStream(
        stream(hostile(base, subclass)),
      );
      try {
        ids.push([...loaded.search("old").ids]);
        await index.addSegment(hostile(delta, subclass));
        ids.push([...index.search("new").ids]);
        await index.replaceSegments([
          { source: stream(hostile(base, subclass)) },
        ]);
        ids.push([...index.search("old").ids]);
      } finally {
        loaded.free();
        index.free();
      }
    }
    let reads = 0;
    const oversized = new Uint8Array(8 * 1024 * 1024);
    Object.defineProperty(oversized, "length", {
      get() {
        return ++reads === 1 ? 0 : 8 * 1024 * 1024;
      },
    });
    let code: string | undefined;
    try {
      api.AsearchIndex.fromBytes(oversized, {
        limits: { maxWorkingBytes: 65536, maxPatternUnits: 8 },
      });
    } catch (error) {
      code = (error as { code: string }).code;
    }
    const part = {
      length: 1,
      ids: Uint32Array.of(1),
      levels: Uint8Array.of(0),
      refs: Uint32Array.of(7),
      orders: Uint32Array.of(0),
    };
    for (const value of [part.ids, part.levels, part.refs, part.orders])
      Object.defineProperty(value, "length", { get: unexpected });
    const merged = [...api.mergeResults([part], 2).ids];
    return { ids, reads, code, merged };
  });
  expect(result).toEqual({
    ids: Array.from({ length: 6 }, () => [0]),
    reads: 0,
    code: "RESOURCE_LIMIT",
    merged: [1],
  });
});
