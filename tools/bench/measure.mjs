import { pairedInterval, quantile } from "./statistics.mjs";

// Category A: already-constructed matcher, fixed level, complete corpus scan.
// Each sample is a batch mean per match, never an individual keystroke latency.
export function measure(baselineClass, candidateClass, data, now) {
  const reports = [];
  for (const pattern of data.patterns) {
    const options = { profile: "asearch.ts-lib" };
    const baseline = new baselineClass(pattern, 1, options);
    let candidate;
    try {
      candidate = new candidateClass(pattern, 1, options);
      for (const text of data.texts)
        for (let e = 0; e <= 3; e++) {
          if (baseline.match(text, e) !== candidate.match(text, e))
            throw new Error("Benchmark semantics differ");
        }
      const scan = (matcher) => {
        let matched = 0;
        for (const text of data.texts)
          matched += Number(matcher.match(text, 3));
        return matched;
      };
      const expected = scan(baseline);
      const verify = (matched, repeats) => {
        if (matched !== expected * repeats)
          throw new Error("Benchmark measured results differ");
      };
      for (let i = 0; i < 10; i++) {
        scan(baseline);
        scan(candidate);
      }
      const batch = (matcher, repeats) => {
        let matched = 0;
        const start = now();
        for (let i = 0; i < repeats; i++) matched += scan(matcher);
        return { elapsed: now() - start, matched };
      };
      let repeats = 1;
      while (repeats < 1024) {
        const a = batch(baseline, repeats);
        const b = batch(candidate, repeats);
        verify(a.matched, repeats);
        verify(b.matched, repeats);
        if (Math.min(a.elapsed, b.elapsed) >= 4) break;
        repeats *= 2;
      }
      const runs = [];
      let checksum = 0;
      for (let run = 0; run < 5; run++) {
        const samples = { baseline: [], candidate: [] };
        for (let sample = 0; sample < 30; sample++) {
          const order =
            (run + sample) % 2
              ? ["candidate", "baseline"]
              : ["baseline", "candidate"];
          for (const name of order) {
            const sample = batch(
              name === "baseline" ? baseline : candidate,
              repeats,
            );
            if (sample.elapsed <= 0)
              throw new Error(
                "Timer resolution is insufficient for this workload",
              );
            verify(sample.matched, repeats);
            checksum += sample.matched;
            samples[name].push(sample.elapsed / (data.texts.length * repeats));
          }
        }
        runs.push(samples);
      }
      reports.push({
        pattern,
        repeats,
        checksum,
        runs,
        interval: pairedInterval(runs),
        summary: Object.fromEntries(
          ["baseline", "candidate"].map((name) => {
            const samples = runs.flatMap((run) => run[name]);
            return [
              name,
              {
                batchMeanMatchMsP50: quantile(samples, 0.5),
                batchMeanMatchMsP95: quantile(samples, 0.95),
                batchMeanMatchMsP99: quantile(samples, 0.99),
              },
            ];
          }),
        ),
      });
    } finally {
      try {
        candidate?.free?.();
      } finally {
        baseline.free?.();
      }
    }
  }
  return reports;
}
