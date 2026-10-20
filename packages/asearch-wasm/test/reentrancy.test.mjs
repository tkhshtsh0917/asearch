import assert from "node:assert/strict";
import test from "node:test";
import { Asearch } from "../dist/node.mjs";

test("matching revalidates handles after caller-controlled conversions", () => {
  for (const phase of ["valueOf", "toString", "query", "levelQuery"]) {
    const a = new Asearch("a");
    const query = new String("a");
    query.toString = () => {
      a.free();
      return "a";
    };
    const level = {
      valueOf() {
        if (phase === "valueOf") a.free();
        return 0;
      },
      toString() {
        a.free();
        return "0";
      },
    };
    assert.throws(
      () =>
        phase === "levelQuery"
          ? a.matchLevel(query)
          : a.match(
              phase === "query" ? query : "a",
              phase === "query" ? 0 : level,
            ),
      { code: "DISPOSED" },
    );
    a.free();
  }
  for (const level of [-1, NaN, 0])
    assert.throws(
      () => Asearch.prototype.match.call({}, "a", level),
      TypeError,
    );
});
