import assert from "node:assert/strict";
import test from "node:test";
import { pairedInterval, quantile, regression } from "./statistics.mjs";

const runs = (ratios) =>
  ratios.map((ratio) => ({
    baseline: Array(30).fill(10),
    candidate: Array(30).fill(10 * ratio),
  }));

test("noise cannot establish a regression", () => {
  assert.equal(regression(runs([0.6, 1.6, 1, 1.2, 0.8])).status, "unconfirmed");
});
test("a repeated >10% regression requires independent confirmation", () => {
  const slow = runs([1.2, 1.21, 1.22, 1.19, 1.2]);
  assert.equal(regression(slow).status, "candidate");
  assert.equal(regression(slow, slow).status, "confirmed");
  assert.equal(regression(slow, runs([1, 1, 1, 1, 1])).status, "unconfirmed");
});
test("aggregation is deterministic and rejects invalid samples", () => {
  const data = runs([1, 1.1, 0.9, 1.05, 1]);
  assert.deepEqual(pairedInterval(data), pairedInterval(data));
  assert.throws(() => pairedInterval(data.slice(1)), RangeError);
  for (const bad of [[], [0], [-1], [NaN], [Infinity]])
    assert.throws(() => quantile(bad, 0.5), RangeError);
});
