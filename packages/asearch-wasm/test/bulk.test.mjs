import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import * as esm from "../dist/node.mjs";

const require = createRequire(import.meta.url);
const cjs = require("../dist/node.cjs");
const root = fileURLToPath(new URL("../../../", import.meta.url));

for (const [name, api] of [
  ["ESM", esm],
  ["CJS", cjs],
]) {
  test(`${name}: bulk ranking, normalization, totals and independent copies`, (t) => {
    const index = api.AsearchIndex.fromDocs(
      [
        { key: 5, order: 10, texts: ["ｶﾞＡ😀", "other"] },
        { key: 2, order: 10, texts: ["がa😀", "がa😀"] },
        { key: 8, order: 1, texts: ["がa😀"] },
      ],
      { limits: { maxPatternUnits: 128 } },
    );
    t.after(() => index.free());
    const result = index.search("ガＡ😀", {
      mode: api.MatchMode.Exact,
      maxAmbig: api.fixed(0),
      countTotal: true,
    });
    assert.deepEqual([...result.ids], [8, 2, 5]);
    assert.deepEqual([...result.orders], [1, 10, 10]);
    assert.deepEqual([...result.totals], [3]);
    assert.deepEqual([...result.levels], [0, 0, 0]);
    assert.equal(result.stats.engine, "transposed-blocked");
    assert.ok(result.stats.blocks > 0);
    const copy = result.copy();
    result.ids[0] = 99;
    assert.equal(copy.ids[0], 8);
    assert.deepEqual(
      [...index.search("ガＡ😀", { limit: 0, countTotal: true }).totals],
      [3],
    );
    assert.equal(copy.copy().ids[0], 8);
    assert.equal(index.search("no such query").totals, undefined);
    const loaded = api.AsearchIndex.fromBytes(index.toBytes(), {
      limits: { maxPatternUnits: 128 },
    });
    t.after(() => loaded.free());
    assert.deepEqual(loaded.toBytes(), index.toBytes());
    assert.deepEqual(
      [...loaded.search("がa😀", { maxAmbig: 0 }).ids],
      [8, 2, 5],
    );
  });

  test(`${name}: bulk levels agree with single-text matching for every profile, mode and K`, (t) => {
    const texts = ["", "abc", "xabcy", "ab", "a c", "😀", "漢字", "漢の字"];
    for (const profile of ["ideal", "asearch.ts", "asearch.ts-lib"]) {
      const index = api.AsearchIndex.fromStrings(texts, {
        normalizer: "none",
        profile,
        limits: { maxPatternUnits: 64 },
      });
      t.after(() => index.free());
      for (const mode of [0, 1, 2])
        for (const k of [0, 1, 2, 3, 4, 7])
          for (const pattern of ["", "abc", "a c", "😀", "漢 字"]) {
            const matcher = new api.Asearch(pattern, mode, { profile });
            t.after(() => matcher.free());
            const expected = texts
              .map((text, key) => ({ key, level: matcher.matchLevel(text, k) }))
              .filter((hit) => hit.level >= 0)
              .sort((a, b) => a.level - b.level || a.key - b.key);
            const result = index.search(pattern, {
              mode,
              wrap: false,
              maxAmbig: k,
            });
            assert.deepEqual(
              [...result.ids],
              expected.map((hit) => hit.key),
            );
            assert.deepEqual(
              [...result.levels],
              expected.map((hit) => hit.level),
            );
          }
    }
  });

  test(`${name}: normalized patterns use host-locale casing and ignore multi-symbol expansions`, () => {
    const texts = ["I", "i", "ı", "İ", "ß", "SS", "S", "Σ", "σ", "ς"];
    const lower = String.prototype.toLocaleLowerCase;
    try {
      String.prototype.toLocaleLowerCase = function () {
        return lower.call(this, "tr");
      };
      for (const profile of ["ideal", "asearch.ts", "asearch.ts-lib"]) {
        const index = api.AsearchIndex.fromStrings(texts, {
          normalizer: "none",
          profile,
        });
        try {
          for (const pattern of ["I", "İ", "ß", "Σ", "ς"]) {
            const matcher = new api.Asearch(pattern, api.MatchMode.Exact, {
              profile,
            });
            try {
              const expected = texts
                .map((text, key) => ({
                  key,
                  level: matcher.matchLevel(text, 0),
                }))
                .filter((hit) => hit.level === 0)
                .map((hit) => hit.key);
              assert.deepEqual(
                [...index.search(pattern, { mode: 0, maxAmbig: 0 }).ids],
                expected,
              );
            } finally {
              matcher.free();
            }
          }
        } finally {
          index.free();
        }
      }
    } finally {
      String.prototype.toLocaleLowerCase = lower;
    }
  });

  test(`${name}: wildcard spaces skip host casing hooks`, () => {
    const index = api.AsearchIndex.fromStrings(["a b"]);
    const lower = String.prototype.toLocaleLowerCase;
    try {
      String.prototype.toLocaleLowerCase = function () {
        if (String(this) === " ") throw new Error("literal space casing");
        return lower.call(this);
      };
      assert.deepEqual([...index.search("a b", { maxAmbig: 0 }).ids], [0]);
      assert.throws(
        () => index.search("a b", { mode: 0, maxAmbig: 0 }),
        /literal space casing/,
      );
    } finally {
      String.prototype.toLocaleLowerCase = lower;
      index.free();
    }
  });

  test(`${name}: casing callbacks cannot reuse a freed native index`, () => {
    const index = api.AsearchIndex.fromStrings(["a"], { normalizer: "none" });
    const lower = String.prototype.toLocaleLowerCase;
    try {
      String.prototype.toLocaleLowerCase = function () {
        index.free();
        return lower.call(this);
      };
      assert.throws(() => index.search("a", { mode: 0, maxAmbig: 0 }), {
        code: "DISPOSED",
      });
    } finally {
      String.prototype.toLocaleLowerCase = lower;
      index.free();
    }
  });

  test(`${name}: casing preserves a surrogate pair split across UTF-16 conversion chunks`, () => {
    const pattern = `${"a".repeat(8191)}😀b`;
    const index = api.AsearchIndex.fromStrings([pattern], {
      normalizer: "none",
      limits: { maxPatternUnits: 9000 },
    });
    try {
      assert.deepEqual(
        [...index.search(pattern, { mode: 0, maxAmbig: 0 }).ids],
        [0],
      );
    } finally {
      index.free();
    }
  });

  test(`${name}: reentrant casing cannot replace the outer prepared query`, () => {
    const index = api.AsearchIndex.fromStrings(["a", "b"], {
      normalizer: "none",
    });
    const lower = String.prototype.toLocaleLowerCase;
    let calls = 0;
    try {
      String.prototype.toLocaleLowerCase = function () {
        calls++;
        assert.throws(() => index.search("b", { mode: 0, maxAmbig: 0 }), {
          code: "CONFLICT",
        });
        return lower.call(this);
      };
      assert.deepEqual(
        [...index.search("a", { mode: 0, maxAmbig: 0 }).ids],
        [0],
      );
      assert.ok(calls > 0);
    } finally {
      String.prototype.toLocaleLowerCase = lower;
    }
    try {
      assert.deepEqual(
        [...index.search("b", { mode: 0, maxAmbig: 0 }).ids],
        [1],
      );
    } finally {
      index.free();
    }
  });

  test(`${name}: input errors and lifecycle are checked without invalidating copied results`, () => {
    const index = api.AsearchIndex.fromStrings(
      ["a", "😀", "\ud800", "a".repeat(40)],
      { normalizer: "none", limits: { maxPatternUnits: 64 } },
    );
    assert.deepEqual(
      [...index.search("a".repeat(40), { mode: 0, maxAmbig: 0 }).ids],
      [3],
    );
    assert.deepEqual(
      [...index.search("\ud800", { mode: 0, maxAmbig: 0 }).ids],
      [2],
    );
    assert.throws(() => index.search("a".repeat(65)), {
      code: "RESOURCE_LIMIT",
    });
    assert.throws(() => index.search("a", { maxAmbig: 8 }), {
      code: "INVALID_INPUT",
    });
    assert.throws(() => index.search("a", { limit: -1 }), {
      code: "INVALID_INPUT",
    });
    assert.throws(() => index.search("a", { osa: true }), {
      code: "INVALID_INPUT",
    });
    assert.throws(
      () => api.AsearchIndex.fromBytes(index.toBytes(), { expected: {} }),
      { code: "INVALID_INPUT" },
    );
    assert.throws(
      () => api.AsearchIndex.fromStrings(["a"], { profile: "unknown" }),
      { code: "INVALID_INPUT" },
    );
    const copy = index.search("a", { maxAmbig: 0 }).copy();
    assert.throws(
      () =>
        index.search("a", {
          get mode() {
            index.free();
            return 0;
          },
        }),
      { code: "DISPOSED" },
    );
    index.free();
    assert.throws(() => index.toBytes(), { code: "DISPOSED" });
    assert.throws(() => index.search("a"), { code: "DISPOSED" });
    assert.ok(copy.length > 0);
    assert.throws(
      () => api.AsearchIndex.prototype.search.call({}, "a"),
      TypeError,
    );
    assert.throws(
      () =>
        api.AsearchIndex.fromDocs([
          { key: 1, texts: ["a"] },
          { key: 1, texts: ["b"] },
        ]),
      { code: "INVALID_INPUT" },
    );
    assert.throws(
      () =>
        api.AsearchIndex.fromStrings(["a"], { limits: { maxWorkingBytes: 1 } }),
      { code: "RESOURCE_LIMIT" },
    );
    assert.throws(() => api.AsearchIndex.fromBytes(new Uint8Array(64)), {
      code: "INVALID_INDEX",
    });
  });
}

test("native CLI and WASM builder produce identical ASIX bytes", async () => {
  const directory = await mkdtemp(join(tmpdir(), "asearch-index-"));
  try {
    const docs = [
      { key: 9, texts: ["ｶﾞＡ😀", "あいうえお"] },
      { key: 1, order: 3, texts: ["ABC", ""] },
    ];
    const input = join(directory, "docs.jsonl");
    const output = join(directory, "index.asix");
    await writeFile(
      input,
      `${docs.map((doc) => JSON.stringify(doc)).join("\n")}\n`,
    );
    const result = spawnSync(
      "cargo",
      [
        "run",
        "--quiet",
        "--locked",
        "-p",
        "asearch-cli",
        "--",
        "build",
        input,
        output,
      ],
      { cwd: root, encoding: "utf8" },
    );
    assert.equal(result.status, 0, result.stderr);
    const native = await readFile(output);
    assert.deepEqual(esm.buildIndex(docs), new Uint8Array(native));
    assert.deepEqual(
      cjs.buildIndex([...docs].reverse()),
      new Uint8Array(native),
    );
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
