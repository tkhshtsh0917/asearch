import assert from "node:assert/strict";
import { createRequire } from "node:module";
import test from "node:test";
import * as esm from "../dist/node.mjs";

const cjs = createRequire(import.meta.url)("../dist/node.cjs");
for (const [name, api] of [
  ["ESM", esm],
  ["CJS", cjs],
]) {
  test(`${name}: sessions match full search through typing, K boundaries and normalization`, () => {
    const index = api.AsearchIndex.fromDocs(
      [
        { key: 1, texts: ["abcd", "abcde"] },
        { key: 2, texts: ["xxabcde"] },
        { key: 3, texts: ["がく", "か"] },
        { key: 4, texts: ["😀ab", "😀ac"] },
      ],
      { limits: { maxPatternUnits: 64 } },
    );
    const sessions = [];
    try {
      for (const options of [
        { countTotal: true, limit: 1 },
        { mode: 0, maxAmbig: 2, countTotal: true },
        { mode: 1, maxAmbig: 0, countTotal: true },
        { countTotal: true, maxCacheBytes: 0 },
        { engine: "transposed-blocked" },
      ]) {
        const session = index.session(options);
        sessions.push(session);
        const { maxCacheBytes, ...searchOptions } = options;
        let reused = false;
        for (const pattern of [
          "",
          "a",
          "ab",
          "abc",
          "abcd",
          "abcde",
          "abc",
          "abx",
          "ｶ",
          "ｶﾞ",
          "ｶﾞｸ",
          "😀",
          "😀a",
          "😀ab",
          "",
        ]) {
          const actual = session.update(pattern);
          const expected = index.search(pattern, searchOptions);
          for (const field of ["ids", "levels", "orders", "refs", "totals"])
            assert.deepEqual(actual[field], expected[field]);
          reused ||= actual.stats.reusedRows > 0;
        }
        assert.equal(
          reused,
          maxCacheBytes !== 0 && options.engine !== "transposed-blocked",
        );
        session.reset();
        assert.equal(session.update("abcd").stats.reusedRows, 0);
        assert.throws(() => session.update("x".repeat(65)), {
          code: "RESOURCE_LIMIT",
        });
        assert.deepEqual(
          session.update("abcd").ids,
          index.search("abcd", searchOptions).ids,
        );
      }
      index.free();
      for (const session of sessions) {
        assert.throws(() => session.update("a"), { code: "DISPOSED" });
        assert.throws(() => session.reset(), { code: "DISPOSED" });
      }
    } finally {
      for (const session of sessions) {
        session.free();
        session.free();
      }
      index.free();
    }
  });

  test(`${name}: session capacity is shared and returned after disposal`, () => {
    const index = api.AsearchIndex.fromStrings(["abc"], {
      limits: { maxWorkingBytes: 700_000, maxPatternUnits: 64 },
    });
    const sessions = [];
    try {
      assert.throws(() => index.session({ maxCacheBytes: null }));
      for (let i = 0; i < 100; i++) {
        try {
          sessions.push(index.session({ maxCacheBytes: 0 }));
        } catch (error) {
          assert.equal(error.code, "RESOURCE_LIMIT");
          break;
        }
      }
      assert.ok(sessions.length > 0 && sessions.length < 100);
      const old = sessions.pop();
      old.free();
      assert.throws(() => old.update("a"), { code: "DISPOSED" });
      const replacement = index.session({ maxCacheBytes: 0 });
      replacement.free();
      assert.deepEqual([...index.search("abc").ids], [0]);
    } finally {
      for (const session of sessions) session.free();
      index.free();
    }
  });

  test(`${name}: session casing failures, reentry and disposal preserve safety`, () => {
    const index = api.AsearchIndex.fromStrings(["abc", "abcd"]);
    const session = index.session();
    const lower = String.prototype.toLocaleLowerCase;
    try {
      session.update("ab");
      String.prototype.toLocaleLowerCase = () => {
        assert.throws(() => session.update("a"), { code: "CONFLICT" });
        assert.throws(() => session.reset(), { code: "CONFLICT" });
        throw new Error("host casing failed");
      };
      assert.throws(() => session.update("abc"), /host casing failed/);
      String.prototype.toLocaleLowerCase = lower;
      assert.deepEqual([...session.update("abc").ids], [0, 1]);
      String.prototype.toLocaleLowerCase = function () {
        session.free();
        return lower.call(this);
      };
      assert.throws(() => session.update("a"), { code: "DISPOSED" });
      String.prototype.toLocaleLowerCase = lower;
      const child = index.session();
      String.prototype.toLocaleLowerCase = function () {
        index.free();
        return lower.call(this);
      };
      assert.throws(() => child.update("a"), { code: "DISPOSED" });
      child.free();
    } finally {
      String.prototype.toLocaleLowerCase = lower;
      session.free();
      index.free();
    }
    assert.throws(
      () => api.SearchSession.prototype.update.call({}, "a"),
      TypeError,
    );
    assert.throws(() => api.SearchSession.prototype.free.call({}), TypeError);
  });
}

for (const [name, api] of [
  ["ESM", esm],
  ["CJS", cjs],
]) {
  test(`${name}: custom ambiguity receives normalized unwrapped context exactly once`, () => {
    for (const profile of ["ideal", "asearch.ts", "asearch.ts-lib"]) {
      const index = api.AsearchIndex.fromStrings(["が😀a", "が😀b"], {
        profile,
      });
      const contexts = [];
      const policy = (context) => {
        contexts.push(context);
        assert.equal(Object.isFrozen(context), true);
        return 1;
      };
      const session = index.session({
        maxAmbig: policy,
        mode: 2,
        countTotal: true,
      });
      try {
        const expected = index.search("ｶﾞ😀Ａ", {
          maxAmbig: 1,
          countTotal: true,
        });
        for (const actual of [
          index.search("ｶﾞ😀Ａ", { maxAmbig: policy, countTotal: true }),
          session.update("ｶﾞ😀Ａ"),
        ]) {
          assert.deepEqual(actual.ids, expected.ids);
          assert.deepEqual(actual.levels, expected.levels);
          assert.deepEqual(actual.totals, expected.totals);
        }
        assert.deepEqual(
          contexts,
          Array.from({ length: 2 }, () => ({
            normalized: "が😀a",
            mode: 2,
            profile,
          })),
        );
        for (const invalid of [
          null,
          undefined,
          NaN,
          Infinity,
          -1,
          8,
          1.5,
          "1",
          Promise.resolve(1),
        ])
          assert.throws(() => index.search("abc", { maxAmbig: () => invalid }));
        assert.throws(
          () =>
            index.search("a", {
              maxAmbig() {
                throw new Error("policy failed");
              },
            }),
          /policy failed/,
        );
        assert.deepEqual(session.update("ｶﾞ😀Ａ").ids, expected.ids);
      } finally {
        session.free();
        index.free();
      }
    }
  });
  test(`${name}: custom ambiguity cannot reuse freed or reentrant handles`, () => {
    const index = api.AsearchIndex.fromStrings(["abc"]);
    const session = index.session({
      maxAmbig() {
        index.free();
        return 0;
      },
    });
    try {
      assert.throws(() => session.update("a"), { code: "DISPOSED" });
    } finally {
      session.free();
      index.free();
    }
    const other = api.AsearchIndex.fromStrings(["abc"]);
    try {
      assert.throws(
        () =>
          other.search("a", {
            maxAmbig() {
              other.search("b");
              return 0;
            },
          }),
        { code: "CONFLICT" },
      );
      assert.deepEqual([...other.search("abc").ids], [0]);
    } finally {
      other.free();
    }
  });
}

test("narrowing retains candidates outside top-N and restores full search as K grows", () => {
  const index = esm.AsearchIndex.fromStrings([
    ...Array(1000).fill("zzzz"),
    "abc",
    "abcdef",
    "abc xyz",
    "abcx",
  ]);
  const options = {
    maxAmbig: ({ normalized }) => (normalized.length >= 5 ? 1 : 0),
    limit: 1,
    countTotal: true,
  };
  const session = index.session(options);
  try {
    session.update("");
    const dense = session.update("ab");
    assert.notEqual(dense.stats.engine, "narrowing");
    assert.deepEqual(dense.ids, index.search("ab", options).ids);
    const narrowed = session.update("abc");
    assert.equal(narrowed.stats.engine, "narrowing");
    assert.equal(narrowed.stats.narrowedTexts, 4);
    assert.equal(session.update("abcd").ids[0], 1001);
    const grown = session.update("abcde");
    assert.notEqual(grown.stats.engine, "narrowing");
    const expected = index.search("abcde", options);
    assert.deepEqual(grown.ids, expected.ids);
    assert.deepEqual(grown.totals, expected.totals);
  } finally {
    session.free();
    index.free();
  }
});

test("a full session cache leaves room for query transport and the complete packed result", () => {
  const index = esm.AsearchIndex.fromStrings(
    Array(1000).fill("a".repeat(200)),
    { limits: { maxPatternUnits: 128, maxWorkingBytes: 8 * 1024 * 1024 } },
  );
  const session = index.session({
    maxCacheBytes: 0xffffffff,
    maxAmbig: 3,
    limit: 1000,
    countTotal: true,
  });
  try {
    session.update("a".repeat(127));
    const result = session.update("Ａ".repeat(128));
    assert.equal(result.length, 1000);
    assert.equal(result.totals[0], 1000);
    assert.deepEqual(
      result.ids,
      index.search("Ａ".repeat(128), {
        maxAmbig: 3,
        limit: 1000,
        countTotal: true,
      }).ids,
    );
  } finally {
    session.free();
    index.free();
  }
});
