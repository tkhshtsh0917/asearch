import assert from "node:assert/strict";
import { createRequire } from "node:module";
import test from "node:test";
import { mergeResults } from "../dist/node.mjs";

function part(rows) {
  return {
    length: rows.length,
    ids: Uint32Array.from(rows, (r) => r[0]),
    levels: Uint8Array.from(rows, (r) => r[1]),
    orders: Uint32Array.from(rows, (r) => r[2]),
    refs: Uint32Array.from(rows, (r) => r[3]),
  };
}
test("static parts merge by rank with key deduplication and shard-owned references", () => {
  const inputs = [
    part([
      [3, 0, 1, 33],
      [1, 1, 0, 11],
      [7, 2, 0, 77],
    ]),
    part([
      [2, 0, 0, 22],
      [3, 0, 1, 333],
      [1, 0, 3, 111],
      [8, 2, 0, 88],
    ]),
  ];
  const result = mergeResults(structuredClone(inputs), 100);
  assert.deepEqual([...result.ids], [2, 3, 1, 7, 8]);
  assert.deepEqual([...result.parts], [1, 0, 1, 0, 1]);
  assert.deepEqual([...result.refs], [22, 33, 111, 77, 88]);
  assert.deepEqual([...mergeResults(inputs, 2).ids], [2, 3]);
  assert.equal(mergeResults(inputs, 0).ids.length, 0);
  assert.equal(mergeResults([], 10).ids.length, 0);
  result.ids[0] = 99;
  assert.equal(inputs[1].ids[0], 2);
});
test("merge rejects malformed or unsorted parts without summing incomplete totals", () => {
  for (const input of [
    null,
    {},
    [null],
    [part([[1, 8, 1, 1]])],
    [
      part([
        [1, 1, 1, 1],
        [2, 0, 2, 2],
      ]),
    ],
    [{ ...part([[1, 0, 0, 1]]), length: 2 }],
    Array(257).fill(part([])),
  ])
    assert.throws(() => mergeResults(input, 10));
  for (const limit of [-1, null, NaN, 1.5])
    assert.throws(() => mergeResults([], limit));
  assert.equal(
    mergeResults([{ ...part([[1, 0, 0, 1]]), totals: Uint32Array.of(1000) }], 1)
      .totals,
    undefined,
  );
});
test("k-way merging matches a flat independent rank/dedup oracle", () => {
  let seed = 31;
  const random = () => (seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0);
  for (let trial = 0; trial < 100; trial++) {
    const groups = Array.from({ length: 1 + (random() % 8) }, () =>
      Array.from({ length: random() % 30 }, () => [
        random() % 25,
        random() % 8,
        random() % 10,
        random(),
      ]).sort((a, b) => a[1] - b[1] || a[2] - b[2] || a[0] - b[0]),
    );
    const all = groups
      .flatMap((group, p) => group.map((row, i) => ({ row, p, i })))
      .sort(
        (a, b) =>
          a.row[1] - b.row[1] ||
          a.row[2] - b.row[2] ||
          a.row[0] - b.row[0] ||
          a.p - b.p ||
          a.i - b.i,
      );
    const seen = new Set(),
      expected = [];
    for (const item of all)
      if (!seen.has(item.row[0])) {
        seen.add(item.row[0]);
        expected.push(item);
      }
    const limit = random() % 30,
      result = mergeResults(groups.map(part), limit);
    assert.deepEqual(
      [...result.ids],
      expected.slice(0, limit).map((x) => x.row[0]),
    );
    assert.deepEqual(
      [...result.refs],
      expected.slice(0, limit).map((x) => x.row[3]),
    );
    assert.deepEqual(
      [...result.parts],
      expected.slice(0, limit).map((x) => x.p),
    );
  }
});

test("part arrays snapshot their bounded length once and ignore custom iterators", () => {
  let reads = 0;
  const input = new Proxy([part([[1, 0, 0, 7]])], {
    get(target, key, receiver) {
      if (key === "length") return ++reads === 1 ? 1 : 257;
      if (key === Symbol.iterator) throw new Error("iterator must not run");
      return Reflect.get(target, key, receiver);
    },
  });
  assert.deepEqual([...mergeResults(input, 1).ids], [1]);
  assert.equal(reads, 1);
});

for (const [name, merge] of [
  ["ESM", mergeResults],
  ["CJS", createRequire(import.meta.url)("../dist/node.cjs").mergeResults],
]) {
  test(`${name}: result arrays use intrinsic lengths despite own getters and subclasses`, () => {
    let reads = 0;
    function hook() {
      reads++;
      return 2;
    }
    for (const subclass of [false, true]) {
      const input = part([[1, 0, 0, 7]]);
      for (const key of ["ids", "levels", "orders", "refs"]) {
        const source = input[key];
        if (subclass) {
          const Base = source.constructor;
          input[key] = new (class extends Base {
            get length() {
              return hook();
            }
          })(source);
        } else {
          Object.defineProperty(source, "length", { get: hook });
        }
      }
      // Claimed lengths must never validate nonexistent array elements.
      assert.throws(() => merge([{ ...input, length: 2 }], 2), {
        code: "INVALID_INPUT",
      });
      assert.deepEqual([...merge([input], 2).ids], [1]);
      assert.deepEqual([...merge([input], 2).refs], [7]);
    }
    assert.equal(reads, 0);
  });
}

for (const [name, merge] of [
  ["ESM", mergeResults],
  ["CJS", createRequire(import.meta.url)("../dist/node.cjs").mergeResults],
]) {
  test(`${name}: later getters cannot invalidate earlier validation`, () => {
    for (const detach of [false, true]) {
      const first = part([[42, 0, 0, 123]]);
      const second = {
        ...part([]),
        get ids() {
          if (detach)
            structuredClone(first.ids.buffer, { transfer: [first.ids.buffer] });
          else first.levels[0] = 255;
          return new Uint32Array();
        },
      };
      assert.throws(() => merge([first, second], 1), { code: "INVALID_INPUT" });
    }
  });
  test(`${name}: actual element types override forged prototypes and tags`, () => {
    for (const key of ["ids", "orders", "refs", "levels"]) {
      const input = part([
        [1, 0, 0, 7],
        [2, 0, 0, 8],
      ]);
      const Base = key === "levels" ? Uint8Array : Uint32Array;
      const forged = Object.setPrototypeOf(
        Int32Array.of(-1, 0),
        Base.prototype,
      );
      Object.defineProperty(forged, Symbol.toStringTag, { value: Base.name });
      input[key] = forged;
      assert.throws(() => merge([input], 2), { code: "INVALID_INPUT" });
    }
  });
}
