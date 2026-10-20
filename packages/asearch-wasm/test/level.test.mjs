import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { Asearch } from "../dist/node.mjs";

const fixture = JSON.parse(
  readFileSync(
    new URL(
      "../../../crates/asearch-rs/tests/fixtures/compat.json",
      import.meta.url,
    ),
    "utf8",
  ),
);
const decode = (units) => String.fromCharCode(...units);
for (const [profile, key] of [
  ["asearch.ts", "src"],
  ["asearch.ts-lib", "lib"],
]) {
  test(`matchLevel finds first accepted level in every ${profile} fixture`, () => {
    for (const c of fixture.cases) {
      if (c.ctor.throws) continue;
      const matcher = new Asearch(
        decode(c.pattern),
        c.mode.nan ? NaN : c.mode.json,
        { profile },
      );
      for (const [i, text] of c.texts.entries()) {
        for (let max = 0; max <= 5; max++) {
          const expected = c[key][i].slice(0, max + 1).findIndex(Boolean);
          assert.equal(
            matcher.matchLevel(decode(text), max),
            expected,
            `${c.id} max=${max}`,
          );
        }
        const expected =
          Array.from({ length: 32 }, (_, e) => e).find((e) =>
            matcher.match(decode(text), e),
          ) ?? -1;
        assert.equal(matcher.matchLevel(decode(text), 31), expected);
      }
      matcher.free();
    }
  });
}
test("matchLevel validates strict bounds and lifecycle", () => {
  const a = new Asearch("a");
  assert.equal(a.matchLevel(""), 1);
  assert.equal(a.matchLevel("", 0), -1);
  assert.equal(a.matchLevel("a"), 0);
  for (const max of [NaN, Infinity, -1, 1.5, 32, 65535])
    assert.throws(() => a.matchLevel("", max), RangeError);
  for (const max of ["1", null, 1n])
    assert.throws(() => a.matchLevel("", max), TypeError);
  assert.throws(() => a.matchLevel(123), TypeError);
  a.free();
  assert.throws(() => a.matchLevel("a"), { code: "DISPOSED" });
});
