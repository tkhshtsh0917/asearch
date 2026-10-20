import assert from "node:assert/strict";
import { createRequire } from "node:module";
import test from "node:test";
import * as esm from "../dist/node.mjs";

const cjs = createRequire(import.meta.url)("../dist/node.cjs");
for (const [name, api] of [
  ["ESM", esm],
  ["CJS", cjs],
]) {
  test(`${name}: intent DAG equals fully expanded bulk results and explains a real expansion`, () => {
    const entries = [
      { key: 2, order: 0, expr: "(ｶ|ﾊ)ﾞ😀?" },
      { key: 1, expr: "(ライト|電球)を(交換|修理)したい" },
      { key: 3, expr: "(a|b)?c" },
    ];
    const docs = [
      { key: 2, order: 0, texts: ["ｶﾞ", "ﾊﾞ", "ｶﾞ😀", "ﾊﾞ😀"] },
      {
        key: 1,
        texts: [
          "ライトを交換したい",
          "ライトを修理したい",
          "電球を交換したい",
          "電球を修理したい",
        ],
      },
      { key: 3, texts: ["c", "ac", "bc"] },
    ];
    for (const profile of ["ideal", "asearch.ts", "asearch.ts-lib"]) {
      const intent = api.IntentIndex.fromEntries(entries, { profile }),
        flat = api.AsearchIndex.fromDocs(docs, { profile });
      try {
        for (const mode of [0, 1, 2])
          for (const pattern of [
            "",
            "が",
            "が😀",
            "電球 修理",
            "c",
            "a",
            "ac",
            "unknown",
          ]) {
            const options = { mode, countTotal: true, maxAmbig: 2, limit: 2 };
            const actual = intent.search(pattern, {
                ...options,
                engine: "dag",
              }),
              expected = flat.search(pattern, options);
            for (const field of ["ids", "levels", "orders", "totals"])
              assert.deepEqual(
                actual[field],
                expected[field],
                `${profile} ${mode} ${pattern}`,
              );
            assert.equal(actual.stats.engine, "dag");
            for (let i = 0; i < actual.length; i++) {
              const explanation = intent.explain(
                actual.refs[i],
                pattern,
                options,
              );
              assert.equal(explanation.level, actual.levels[i]);
              assert.ok(
                docs
                  .find((doc) => doc.key === actual.ids[i])
                  .texts.includes(explanation.text),
              );
              assert.equal(explanation.coordinateSpace, "original-utf16");
            }
          }
      } finally {
        intent.free();
        flat.free();
      }
    }
  });
  test(`${name}: exponentially many choices stay bounded and references stay owner-specific`, () => {
    const intent = api.IntentIndex.fromEntries([
      { key: 1, expr: "(a|b)".repeat(50) },
    ]);
    const other = api.IntentIndex.fromEntries([{ key: 1, expr: "a" }]);
    try {
      const pattern = "a".repeat(50),
        options = { mode: 0, maxAmbig: 0 };
      const result = intent.search(pattern, options);
      assert.equal(result.length, 1);
      assert.equal(
        intent.explain(result.refs[0], pattern, options).text,
        pattern,
      );
      assert.throws(() => other.explain(result.refs[0], "a"), {
        code: "INVALID_INPUT",
      });
      assert.equal(intent.explain(result.refs[0], "z", options), null);
      const copy = result.copy();
      other.free();
      assert.deepEqual(copy.ids, result.ids);
      intent.free();
      assert.throws(() => intent.search(pattern), { code: "DISPOSED" });
    } finally {
      other.free();
      intent.free();
      intent.free();
    }
  });
  test(`${name}: grammar, quantities, engine and reentrant callbacks are validated`, () => {
    for (const expr of ["(", ")", "?", "a??", "a*", "a+", "\\"])
      assert.throws(() => api.IntentIndex.fromEntries([{ key: 1, expr }]), {
        code: "INVALID_INPUT",
      });
    assert.throws(
      () =>
        api.IntentIndex.fromEntries([
          { key: 1, expr: `${"(".repeat(65)}a${")".repeat(65)}` },
        ]),
      { code: "RESOURCE_LIMIT" },
    );
    assert.throws(() =>
      api.IntentIndex.fromEntries([
        { key: 1, expr: "a" },
        { key: 1, expr: "b" },
      ]),
    );
    assert.throws(() => api.IntentIndex.fromEntries([{ key: -1, expr: "a" }]));
    assert.throws(() =>
      api.IntentIndex.fromEntries([{ key: 1, expr: new String("a") }]),
    );
    const index = api.IntentIndex.fromEntries([{ key: 1, expr: "a|b" }]);
    try {
      assert.throws(() => index.search("a", { engine: "transposed-blocked" }), {
        code: "UNSUPPORTED_OPTION",
      });
      assert.throws(() => index.explain(0, "a", { original: "a" }));
      let calls = 0;
      const result = index.search("Ａ", {
        maxAmbig(context) {
          calls++;
          assert.equal(context.normalized, "a");
          assert.throws(() => index.search("b"), { code: "CONFLICT" });
          return 0;
        },
      });
      assert.equal(calls, 1);
      assert.deepEqual([...result.ids], [1]);
      assert.throws(
        () =>
          index.search("a", {
            maxAmbig() {
              index.free();
              return 0;
            },
          }),
        { code: "DISPOSED" },
      );
    } finally {
      index.free();
    }
    const empty = api.IntentIndex.fromEntries([]);
    try {
      assert.equal(empty.search("a").length, 0);
    } finally {
      empty.free();
    }
    assert.throws(
      () => api.IntentIndex.prototype.search.call({}, "a"),
      TypeError,
    );
  });
}

test("intent uses host locale casing and bounded indexed entry snapshots", () => {
  let lengths = 0;
  const entries = new Proxy(
    [
      { key: 0, expr: "I" },
      { key: 1, expr: "i" },
      { key: 2, expr: "ı" },
    ],
    {
      get(target, key, receiver) {
        if (key === "length") {
          lengths++;
          return lengths === 1 ? 3 : 10000000;
        }
        if (key === Symbol.iterator)
          throw new Error("iterator must not be used");
        return Reflect.get(target, key, receiver);
      },
    },
  );
  const index = esm.IntentIndex.fromEntries(entries, { normalizer: "none" });
  const lower = String.prototype.toLocaleLowerCase;
  try {
    assert.equal(lengths, 1);
    String.prototype.toLocaleLowerCase = function () {
      return lower.call(this, "tr");
    };
    const result = index.search("I", { mode: 0, maxAmbig: 0 });
    assert.deepEqual([...result.ids], [0, 2]);
    assert.equal(
      index.explain(result.refs[1], "I", { mode: 0, maxAmbig: 0 }).text,
      "ı",
    );
    String.prototype.toLocaleLowerCase = function () {
      index.free();
      return lower.call(this);
    };
    assert.throws(() => index.search("I"), { code: "DISPOSED" });
  } finally {
    String.prototype.toLocaleLowerCase = lower;
    index.free();
  }
});
