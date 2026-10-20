import assert from "node:assert/strict";
import test from "node:test";
import * as api from "../../packages/asearch-wasm/dist/node.mjs";
import { measureSegments } from "./segments-measure.mjs";

test("segment benchmark checks every column against the same flat index", async () => {
  const report = await measureSegments(api, {
    documents: 32,
    runs: 1,
    rounds: 1,
    warmup: 1,
    lifecycleRepeats: 3,
  });
  assert.equal(report.reports.length, 8);
  for (const [i, entry] of report.reports.entries()) {
    assert.equal(entry.segments, i + 1);
    assert.equal(entry.lifecycle.load.samples, 3);
    assert.equal(entry.lifecycle.compact.samples, 3);
    assert.equal(entry.lifecycle.update.samples, 3);
    assert.equal(entry.baseline.samples, report.options.inputs.length);
    assert.equal(entry.candidate.samples, report.options.inputs.length);
    assert.ok(
      entry.pairedRuns.every((run) =>
        [...run.baseline, ...run.candidate].every(
          (n) => Number.isFinite(n) && n >= 0,
        ),
      ),
    );
  }
  await assert.rejects(
    measureSegments(api, { documents: Infinity }),
    RangeError,
  );
});
