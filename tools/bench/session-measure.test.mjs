import assert from "node:assert/strict";
import test from "node:test";
import { measureSessions } from "./session-measure.mjs";

function fixture({ mismatch = false } = {}) {
  const released = [];
  return {
    released,
    api: {
      AsearchIndex: {
        fromBytes() {
          return {
            session(options) {
              const candidate = options.engine === undefined;
              return {
                reset() {},
                free() {
                  released.push(candidate ? "candidate" : "baseline");
                },
                update(pattern) {
                  return {
                    ids: Uint32Array.of(
                      mismatch && candidate ? 2 : pattern.length,
                    ),
                    levels: Uint8Array.of(0),
                    orders: Uint32Array.of(0),
                    totals: Uint32Array.of(1),
                    stats: {
                      engine: candidate ? "incremental" : "transposed-blocked",
                    },
                  };
                },
              };
            },
            free() {
              released.push("index");
            },
          };
        },
      },
    },
  };
}
test("individual session samples retain timer zeros and both paired paths", () => {
  const { api, released } = fixture();
  const result = measureSessions(
    api,
    new Uint8Array(),
    { runs: 5, rounds: 2, warmup: 1, inputs: [["a", "ab"]] },
    () => 0,
  );
  assert.equal(result.baseline.samples, 20);
  assert.equal(result.candidate.zeroSamples, 20);
  assert.equal(result.candidate.p95Ms, 0);
  assert.equal(result.engines.incremental, 20);
  assert.deepEqual(released, ["candidate", "baseline", "index"]);
});
test("a result mismatch or invalid workload cannot produce timing evidence", () => {
  const { api, released } = fixture({ mismatch: true });
  assert.throws(
    () =>
      measureSessions(api, new Uint8Array(), {
        runs: 1,
        rounds: 1,
        warmup: 1,
        inputs: [["a"]],
      }),
    /mismatch/,
  );
  assert.deepEqual(released, ["candidate", "baseline", "index"]);
  assert.throws(
    () => measureSessions(api, new Uint8Array(), { inputs: [[]] }),
    RangeError,
  );
});
