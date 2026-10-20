import assert from "node:assert/strict";
import test from "node:test";
import { measure } from "./measure.mjs";

const data = { patterns: ["a"], texts: ["a"] };

test("coarse clocks calibrate batches without reporting zero durations", () => {
  let time = 0;
  class Matcher {
    match() {
      time += 0.25;
      return true;
    }
  }
  const [result] = measure(Matcher, Matcher, data, () => Math.floor(time));
  assert.equal(result.repeats, 16);
  assert.equal(result.summary.candidate.batchMeanMatchMsP50, 0.25);
  assert.equal(result.interval.ratio, 1);
  assert.equal(result.runs.length, 5);
  assert.ok(
    result.runs.every(
      (run) => run.baseline.length === 30 && run.candidate.length === 30,
    ),
  );
});
test("a frozen clock and unequal semantics cannot produce a benchmark", () => {
  class Yes {
    match() {
      return true;
    }
  }
  class No {
    match() {
      return false;
    }
  }
  assert.throws(() => measure(Yes, No, data, () => 0), /semantics differ/);
  assert.throws(() => measure(Yes, Yes, data, () => 0), /Timer resolution/);
});

test("both handles are released after success or semantic failure", () => {
  let released = 0;
  let time = 0;
  class Matcher {
    match() {
      time += 1;
      return true;
    }
    free() {
      released++;
    }
  }
  class Different extends Matcher {
    match() {
      return false;
    }
  }
  measure(Matcher, Matcher, data, () => time);
  assert.equal(released, 2);
  assert.throws(
    () => measure(Matcher, Different, data, () => time),
    /semantics differ/,
  );
  assert.equal(released, 4);
  assert.throws(
    () => measure(Matcher, Matcher, data, () => 0),
    /Timer resolution/,
  );
  assert.equal(released, 6);
  class BrokenConstructor {
    constructor() {
      throw new Error("construction failed");
    }
  }
  assert.throws(
    () => measure(Matcher, BrokenConstructor, data, () => time),
    /construction failed/,
  );
  assert.equal(released, 7);
});

test("changed results after preflight invalidate timing samples", () => {
  let time = 0;
  class Baseline {
    match() {
      time++;
      return true;
    }
  }
  class Changed {
    calls = 0;
    match() {
      time++;
      return ++this.calls <= 4;
    }
  }
  assert.throws(
    () => measure(Baseline, Changed, data, () => time),
    /measured results differ/,
  );
});
