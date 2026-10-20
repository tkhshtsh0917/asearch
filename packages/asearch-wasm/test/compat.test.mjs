import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import test from "node:test";
import vm from "node:vm";
import { Asearch, init, MatchMode, ready } from "../dist/node.mjs";

const fixture = JSON.parse(
  readFileSync(
    new URL(
      "../../../crates/asearch-rs/tests/fixtures/compat.json",
      import.meta.url,
    ),
  ),
);
const decode = (a) => String.fromCharCode(...a);
const mod = { exports: {} };
vm.runInNewContext(
  readFileSync(
    new URL(
      "../../../vendor/upstream/asearch.ts/lib/index.js",
      import.meta.url,
    ),
    "utf8",
  ),
  { module: mod, exports: mod.exports, process: { env: {} } },
);
const Up = mod.exports;
function outcome(fn) {
  try {
    return { value: fn() };
  } catch (e) {
    return { error: e.name };
  }
}

for (const [profile, key] of [
  ["asearch.ts", "src"],
  ["asearch.ts-lib", "lib"],
]) {
  test(`all ${fixture.cases.length} vendored fixtures: ${profile}`, () => {
    for (const c of fixture.cases) {
      const pattern = decode(c.pattern);
      const mode = c.mode.nan ? NaN : c.mode.json;
      if (c.ctor.throws) {
        assert.throws(() => new Asearch(pattern, mode, { profile }), {
          name: "Error",
          message: c.ctor.throws.message,
        });
        continue;
      }
      const a = new Asearch(pattern, mode, { profile });
      assert.deepEqual(
        Object.keys(a.masks).map((k) => [
          Array.from({ length: k.length }, (_, i) => k.charCodeAt(i)),
          a.masks[k],
        ]),
        c.ctor.ok.masks,
      );
      for (const [ti, t] of c.texts.entries())
        for (const [ei, e] of fixture.ambigs.entries()) {
          assert.equal(
            a.match(decode(t), e),
            c[key][ti][ei],
            `${c.id}/${key}/${ti}/${e}`,
          );
        }
    }
  });
}

test("runtime boundary coercions preserve upstream behavior", () => {
  const ambigs = [
    0,
    1,
    "1",
    [1],
    [0],
    null,
    true,
    1.5,
    -1,
    -0,
    NaN,
    Infinity,
    1e21,
    "01",
    " 1",
    31,
    32,
    33,
    65535,
    { valueOf: () => 1, toString: () => "1" },
    {
      valueOf: () => {
        throw new SyntaxError("test");
      },
    },
    1n,
    Symbol("x"),
  ];
  for (const p of ["", "a", "a b", "😀"])
    for (const mode of [
      0,
      1,
      2,
      3,
      "1",
      null,
      NaN,
      "Include",
      undefined,
      1.0,
    ]) {
      const a = new Asearch(p, mode, { profile: "asearch.ts-lib" });
      const b = new Up.Asearch(p, mode);
      for (const e of ambigs)
        for (const t of ["", p])
          assert.deepEqual(
            outcome(() => a.match(t, e)),
            outcome(() => b.match(t, e)),
            `${p}/${mode}/${String(e)}`,
          );
    }
});

test("Symbol-valued property keys preserve upstream conversion order", () => {
  for (const profile of ["asearch.ts", "asearch.ts-lib"]) {
    const matcher = new Asearch("a", 0, { profile });
    const upstream = new Up.Asearch("a");
    const expected = [];
    const actual = [];
    function level(log) {
      return {
        [Symbol.toPrimitive](hint) {
          log.push(hint);
          return hint === "string" ? Symbol("level") : 0;
        },
      };
    }
    assert.equal(
      matcher.match("", level(actual)),
      upstream.match("", level(expected)),
    );
    assert.deepEqual(actual, ["default", "string"]);
    assert.deepEqual(actual, expected);
  }
});

test("public shape, String objects and MAXLENGTH mutation", () => {
  assert.deepEqual(Object.keys(MatchMode), Object.keys(Up.MatchMode));
  for (const p of ["a", new String("a")]) {
    const a = new Asearch(p);
    const b = new Up.Asearch(p);
    assert.deepEqual(Object.keys(a), Object.keys(b));
    for (const key of Object.keys(a)) {
      if (key === "masks")
        assert.deepEqual(Object.entries(a[key]), Object.entries(b[key]));
      else assert.equal(a[key], b[key]);
    }
    assert.equal(a.match(new String("A")), true);
  }
  try {
    Asearch.MAXLENGTH = 7;
    Up.Asearch.MAXLENGTH = 7;
    assert.deepEqual(
      outcome(() => new Asearch("abcdefgh")),
      outcome(() => new Up.Asearch("abcdefgh")),
    );
    assert.equal(
      new Asearch("abc").match("abc"),
      new Up.Asearch("abc").match("abc"),
    );
  } finally {
    Asearch.MAXLENGTH = 31;
    Up.Asearch.MAXLENGTH = 31;
  }
});

test("documented deviations are deliberate", () => {
  assert.equal(new Asearch("😀").match("😀"), true);
  assert.equal(
    new Asearch("😀", 0, { profile: "asearch.ts-lib" }).match("😀"),
    false,
  );
  for (const p of [
    123,
    [],
    null,
    undefined,
    { [Symbol.toStringTag]: "String" },
  ])
    assert.throws(() => new Asearch(p), TypeError);
  const a = new Asearch("a");
  assert.throws(() => a.match(123), TypeError);
  assert.throws(() => a.match({ [Symbol.toStringTag]: "String" }), TypeError);
  assert.equal(
    new Asearch(vm.runInNewContext('new String("a")')).match("A"),
    true,
  );
  assert.throws(() => a.match("", 65536), RangeError);
  assert.equal(new Asearch("a".repeat(31)).match("", "length"), false);
  a.masks.a = 0;
  assert.equal(a.match("a"), true);
  for (const profile of ["unknown", 256, null])
    assert.throws(() => new Asearch("a", 0, { profile }), TypeError);
  assert.throws(
    () => new Asearch("a".repeat(32)),
    (e) =>
      Object.getPrototypeOf(e) === Error.prototype &&
      e.message ===
        `Pattern must be shorter than 32 chars. But given ${"a".repeat(32)}`,
  );
});

test("Node ESM and CJS initialize synchronously", async () => {
  await ready;
  assert.equal(init(), ready);
  await init(new Uint8Array());
  const cjs = createRequire(import.meta.url)("../dist/node.cjs");
  assert.equal(new cjs.Asearch("a").match("A"), true);
});

test("README English/Japanese examples retain their assertions", () => {
  assert.equal(new Asearch("hello").match("helo"), false);
  assert.equal(new Asearch("hello").match("helo", 1), true);
  assert.equal(new Asearch("hello").match("helo", 2), true);
  assert.equal(new Asearch("cat", MatchMode.Include).match("I am a cat"), true);
  assert.equal(
    new Asearch("猫である", MatchMode.Include).match("吾輩は猫である。"),
    true,
  );
  assert.equal(new Asearch("asearch").match("search", 1), true);
  assert.equal(new Asearch("asearch").match("search", 2), true);
});
