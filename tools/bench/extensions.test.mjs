import assert from "node:assert/strict";
import test from "node:test";
import { extensions } from "./extensions.mjs";

test("extension measurements distinguish minimum levels from construction", () => {
  let time = 0;
  class Baseline {
    constructor() {
      time++;
    }
    match(text, level) {
      time++;
      return text === "a" || level >= 1;
    }
  }
  class Candidate extends Baseline {
    matchLevel(text, max) {
      time++;
      return text === "a" ? 0 : max >= 1 ? 1 : -1;
    }
    free() {
      time++;
    }
  }
  const data = { patterns: ["a"], texts: ["a", ""] };
  const report = extensions(Baseline, Candidate, data, () => time);
  for (const category of ["minimumLevels", "construction"]) {
    assert.equal(report[category].results.length, 1);
    assert.equal(report[category].results[0].runs.length, 5);
    assert.ok(
      report[category].results[0].summary.candidate.batchMeanOperationMsP50 > 0,
    );
  }
  class Broken extends Candidate {
    matchLevel() {
      return 3;
    }
  }
  assert.throws(
    () => extensions(Baseline, Broken, data, () => time),
    /semantics differ/,
  );
});
