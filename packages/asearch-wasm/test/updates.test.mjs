import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createRequire } from "node:module";
import test from "node:test";
import * as esm from "../dist/node.mjs";

const cjs = createRequire(import.meta.url)("../dist/node.cjs");
function descriptor(bytes) {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  return {
    id: view.getBigUint64(24, true).toString(16).padStart(16, "0"),
    url: "segment.asix",
    bytes: bytes.length,
    texts: view.getUint32(36, true),
    integrity: `sha384-${createHash("sha384").update(bytes).digest("base64")}`,
  };
}
function chunks(bytes, size = 3) {
  let offset = 0;
  return new ReadableStream({
    pull(controller) {
      if (offset === bytes.length) controller.close();
      else {
        controller.enqueue(bytes.subarray(offset, offset + size));
        offset = Math.min(bytes.length, offset + size);
      }
    },
  });
}
for (const [name, api] of [
  ["ESM", esm],
  ["CJS", cjs],
]) {
  test(`${name}: updates apply tombstones, preserve sessions and compact identically`, async () => {
    const first = [
      { key: 1, texts: ["abc"] },
      { key: 2, texts: ["old"] },
      { key: 3, texts: ["removed"] },
    ];
    const replacement = { key: 2, order: 0, texts: ["abcdef", "ｶﾞ😀"] };
    const base = api.buildIndex(first),
      delta = api.buildDelta([replacement], [2, 3]);
    const index = api.AsearchIndex.fromBytes(base),
      session = index.session({ countTotal: true });
    try {
      session.update("ab");
      const before = index.search("abc").refs[0];
      const info = await index.addSegment(chunks(delta), descriptor(delta));
      assert.deepEqual(info, {
        id: descriptor(delta).id,
        bytes: delta.length,
        texts: 2,
        dead: 0,
      });
      assert.equal(index.segments()[0].dead, 2);
      assert.equal(index.segments().length, 2);
      assert.notEqual(index.search("abc").refs[0], before);
      for (const pattern of ["abc", "abcd", "ｶﾞ😀", "old", "removed", ""]) {
        const actual = session.update(pattern),
          expected = index.search(pattern, { countTotal: true });
        assert.deepEqual(actual.ids, expected.ids);
        assert.deepEqual(actual.totals, expected.totals);
        assert.equal(actual.stats.segments, 2);
      }
      assert.deepEqual(
        index.toBytes(),
        api.buildIndex([first[0], replacement]),
      );
      await index.replaceSegments([{ source: base }]);
      assert.deepEqual(index.toBytes(), base);
      assert.deepEqual([...session.update("removed").ids], [3]);
      await index.replaceSegments([
        { source: base },
        { source: delta, expected: descriptor(delta) },
      ]);
      assert.deepEqual(
        index.toBytes(),
        api.buildIndex([first[0], replacement]),
      );
    } finally {
      session.free();
      index.free();
    }
  });
}

for (const [name, api] of [
  ["ESM", esm],
  ["CJS", cjs],
]) {
  test(`${name}: failures retain old bytes and refs and release the update guard`, async () => {
    const index = api.AsearchIndex.fromStrings(["old"]);
    const before = index.toBytes(),
      ref = index.search("old").refs[0];
    const valid = api.buildDelta([{ key: 0, texts: ["new"] }], [0]);
    const corrupt = valid.slice();
    corrupt[corrupt.length - 1] ^= 1;
    try {
      for (const source of [
        corrupt,
        valid.subarray(0, 63),
        new Uint8Array([...valid, 1]),
        api.buildDelta([{ key: 0, texts: ["duplicate"] }], []),
      ]) {
        await assert.rejects(index.addSegment(source), {
          code: "INVALID_INDEX",
        });
        assert.deepEqual(index.toBytes(), before);
        assert.equal(index.search("old").refs[0], ref);
      }
      await assert.rejects(
        index.addSegment(valid, {
          ...descriptor(valid),
          integrity: `sha384-${"A".repeat(64)}`,
        }),
        { code: "INVALID_INDEX" },
      );
      await assert.rejects(index.replaceSegments([]));
      await assert.rejects(
        index.replaceSegments([{ source: before }, { source: before }]),
      );
      await index.addSegment(valid);
      assert.deepEqual([...index.search("new").ids], [0]);
    } finally {
      index.free();
    }
  });
  test(`${name}: staging permits old searches, refuses a second update and aborts on free`, async () => {
    const index = api.AsearchIndex.fromStrings(["old"]),
      session = index.session();
    const valid = api.buildDelta([{ key: 0, texts: ["new"] }], [0]);
    let controller,
      cancelled = false;
    const body = new ReadableStream({
      start(value) {
        controller = value;
        value.enqueue(valid.subarray(0, 64));
      },
      cancel() {
        cancelled = true;
        return new Promise(() => {});
      },
    });
    const pending = index.addSegment(body);
    await Promise.resolve();
    await Promise.resolve();
    assert.deepEqual([...index.search("old").ids], [0]);
    assert.deepEqual([...session.update("old").ids], [0]);
    await assert.rejects(index.addSegment(valid), { code: "BUSY" });
    controller.enqueue(valid.subarray(64));
    controller.close();
    await pending;
    assert.deepEqual([...session.update("new").ids], [0]);
    let second;
    const hanging = new ReadableStream({
      start(value) {
        second = value;
        value.enqueue(valid.subarray(0, 64));
      },
      cancel() {
        cancelled = true;
        return new Promise(() => {});
      },
    });
    const aborted = index.addSegment(hanging);
    await Promise.resolve();
    index.free();
    await assert.rejects(aborted, { code: "DISPOSED" });
    assert.equal(cancelled, true);
    assert.equal(hanging.locked, false);
    assert.throws(() => session.update("new"), { code: "DISPOSED" });
    session.free();
    assert.ok(second);
  });
  test(`${name}: staged headers respect aggregate counts before reading their body`, async () => {
    const index = api.AsearchIndex.fromStrings(["old"], {
      limits: { maxTexts: 1 },
    });
    const valid = api.buildDelta([{ key: 0, texts: ["new"] }], [0]);
    const body = new ReadableStream({
      start(controller) {
        controller.enqueue(valid.subarray(0, 64));
      },
    });
    try {
      await assert.rejects(index.addSegment(body), { code: "RESOURCE_LIMIT" });
      assert.equal(body.locked, false);
      assert.deepEqual([...index.search("old").ids], [0]);
      await index.addSegment(api.buildDelta([], [0]));
      assert.equal(index.search("old").length, 0);
    } finally {
      index.free();
    }
  });
}
