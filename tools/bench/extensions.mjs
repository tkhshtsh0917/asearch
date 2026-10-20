import { measure } from "./measure.mjs";

function minimumLevels(Class, native) {
  return class {
    constructor(pattern, mode, options) {
      this.matcher = new Class(pattern, mode, options);
    }
    match(text, max) {
      if (native) return this.matcher.matchLevel(text, max) + 1;
      for (let e = 0; e <= max; e++)
        if (this.matcher.match(text, e)) return e + 1;
      return 0;
    }
    free() {
      this.matcher.free?.();
    }
  };
}

function construction(Class) {
  return class {
    constructor(pattern, mode, options) {
      this.pattern = pattern;
      this.mode = mode;
      this.options = options;
    }
    match() {
      const matcher = new Class(this.pattern, this.mode, this.options);
      this.last = matcher;
      matcher.free?.();
      return 1;
    }
  };
}

function operations(reports) {
  return reports.map((report) => ({
    ...report,
    summary: Object.fromEntries(
      Object.entries(report.summary).map(([name, values]) => [
        name,
        Object.fromEntries(
          Object.entries(values).map(([key, value]) => [
            key.replace("Match", "Operation"),
            value,
          ]),
        ),
      ]),
    ),
  }));
}

export function extensions(baseline, candidate, data, now) {
  return {
    minimumLevels: {
      category:
        "minimum level per text, without key ranking; repeated JS match vs one WASM matchLevel",
      results: operations(
        measure(
          minimumLevels(baseline, false),
          minimumLevels(candidate, true),
          data,
          now,
        ),
      ),
    },
    construction: {
      category:
        "construction and optional explicit free; upstream GC is deferred and not guaranteed within the sample",
      results: operations(
        measure(construction(baseline), construction(candidate), data, now),
      ),
    },
  };
}
