import assert from "node:assert/strict";
import { createRequire } from "node:module";
import test from "node:test";
import { Asearch } from "../dist/node.mjs";

for (const [name, Class] of [
  ["ESM", Asearch],
  ["CJS", createRequire(import.meta.url)("../dist/node.cjs").Asearch],
]) {
  test(`${name}: explicit release is idempotent and invalidates matching`, () => {
    const matcher = new Class("hello");
    const other = new Class("hello");
    assert.equal(matcher.match("helo", 1), true);
    matcher.free();
    matcher.free();
    matcher[Symbol.dispose ?? Symbol.for("Symbol.dispose")]();
    assert.throws(() => matcher.match("hello"), { code: "DISPOSED" });
    // Disposal takes precedence over input coercion; no user hooks run.
    assert.throws(
      () =>
        matcher.match("hello", {
          valueOf() {
            throw new Error("unexpected coercion");
          },
        }),
      { code: "DISPOSED" },
    );
    assert.equal(other.match("hello"), true);
    other.free();
  });
  test(`${name}: using disposal and invalid receivers`, () => {
    const matcher = new Class("a");
    matcher[Symbol.dispose ?? Symbol.for("Symbol.dispose")]();
    assert.throws(() => matcher.match("a"), { code: "DISPOSED" });
    assert.throws(() => Class.prototype.free.call({}), TypeError);
    assert.deepEqual(Object.keys(matcher), [
      "loopMask",
      "masks",
      "matchMode",
      "pattern",
      "initPattern",
      "acceptPattern",
    ]);
  });
}
