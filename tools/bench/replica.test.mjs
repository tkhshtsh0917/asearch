import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { JsReplica } from "./replica.mjs";

test("benchmark JS replica preserves every published-profile match fixture", () => {
  const fixture = JSON.parse(
    readFileSync(
      new URL(
        "../../crates/asearch-rs/tests/fixtures/compat.json",
        import.meta.url,
      ),
      "utf8",
    ),
  );
  const decode = (units) => String.fromCharCode(...units);
  for (const c of fixture.cases) {
    const make = () =>
      new JsReplica(decode(c.pattern), c.mode.nan ? NaN : c.mode.json);
    if (c.ctor.throws) {
      assert.throws(make);
      continue;
    }
    const matcher = make();
    for (const [i, text] of c.texts.entries()) {
      for (const [j, level] of fixture.ambigs.entries()) {
        assert.equal(
          matcher.match(decode(text), level),
          c.lib[i][j],
          `${c.id} level=${level}`,
        );
      }
    }
  }
});
