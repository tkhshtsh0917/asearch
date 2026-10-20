import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { Asearch, MatchMode } from "../dist/node.mjs";

test("Ideal minimum levels agree with all source-profile fixtures", () => {
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
  for (const c of fixture.cases) {
    if (c.ctor.throws) continue;
    const matcher = new Asearch(
      decode(c.pattern),
      c.mode.nan ? NaN : c.mode.json,
      { profile: "ideal" },
    );
    for (const [i, text] of c.texts.entries()) {
      for (let max = 0; max <= 5; max++) {
        const expected = c.src[i].slice(0, max + 1).findIndex(Boolean);
        assert.equal(
          matcher.matchLevel(decode(text), max),
          expected,
          `${c.id} max=${max}`,
        );
        assert.equal(
          matcher.match(decode(text), max),
          expected >= 0,
          `${c.id} level=${max}`,
        );
      }
    }
    matcher.free();
  }
});

test("Ideal avoids nonmonotonic and shift-wrapping compatibility states", () => {
  for (const mode of [
    MatchMode.Exact,
    MatchMode.Include,
    MatchMode.WildcardSpace,
  ]) {
    const matcher = new Asearch("a", mode, { profile: "ideal" });
    assert.equal(matcher.match("", 0), false);
    for (const level of [1, 2, 3, 31, 32, 33, 65535]) {
      assert.equal(matcher.match("", level), true);
    }
    matcher.free();
    assert.throws(() => matcher.matchLevel("a"), { code: "DISPOSED" });
  }
  const pattern = "a".repeat(31);
  const matcher = new Asearch(pattern, MatchMode.Exact, { profile: "ideal" });
  assert.equal(matcher.match("", 30), false);
  assert.equal(matcher.match("", 31), true);
  assert.equal(matcher.match("", 32), true);
  assert.throws(() => new Asearch("a".repeat(32), 0, { profile: "ideal" }));
  matcher.free();
});
