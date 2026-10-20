import {
  measureLifecycle,
  sameBytes,
  updateColumn,
} from "./segments-lifecycle.mjs";

// Deterministic synthetic corpus isolates segment overhead, not relevance.
function fingerprint(result) {
  return JSON.stringify(
    [result.ids, result.levels, result.orders, result.totals].map((part) => [
      ...part,
    ]),
  );
}
function summary(raw) {
  const sorted = [...raw].sort((a, b) => a - b);
  return {
    samples: raw.length,
    zeroSamples: raw.filter((n) => n === 0).length,
    p50Ms: sorted[Math.floor((sorted.length - 1) * 0.5)],
    p95Ms: sorted[Math.floor((sorted.length - 1) * 0.95)],
    p99Ms: sorted[Math.floor((sorted.length - 1) * 0.99)],
  };
}
async function descriptor(bytes) {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-384", bytes));
  return {
    id: view.getBigUint64(24, true).toString(16).padStart(16, "0"),
    url: "synthetic.asix",
    bytes: bytes.length,
    texts: view.getUint32(36, true),
    integrity: `sha384-${btoa(String.fromCharCode(...digest))}`,
  };
}
export async function measureSegments(
  api,
  {
    documents = 10000,
    runs = 5,
    rounds = 20,
    warmup = 5,
    lifecycleRepeats = 30,
  } = {},
  memoryBytes = () => null,
) {
  if (
    ![documents, runs, rounds, warmup, lifecycleRepeats].every(
      (n) => Number.isInteger(n) && n > 0,
    ) ||
    documents < 8 ||
    documents > 100000 ||
    lifecycleRepeats > 1000 ||
    runs > 100 ||
    rounds > 1000 ||
    warmup > 100
  )
    throw new RangeError("Invalid segment workload");
  const docs = Array.from({ length: documents }, (_, key) => ({
    key,
    texts: [`質問 ${key} 日本の歴史と地理の使い方`, `question ${key} abcdef`],
  }));
  const base = api.buildIndex(docs),
    baseExpected = await descriptor(base);
  const indexOptions = { limits: { maxPatternUnits: 64 } };
  const baseline = api.AsearchIndex.fromBytes(base, {
    ...indexOptions,
    expected: baseExpected,
  });
  const inputs = [
    "",
    "a",
    "ab",
    "abc",
    "abcd",
    "abcde",
    "日本",
    "日本の",
    "日本の歴",
    "日本の歴史",
    "質問 123",
    "ｶﾞ",
    "zzzz",
  ];
  const searchOptions = { maxAmbig: 1, countTotal: true, limit: 30 };
  const expected = inputs.map((pattern) =>
    fingerprint(baseline.search(pattern, searchOptions)),
  );
  const reports = [];
  try {
    for (let segments = 1; segments <= 8; segments++) {
      const sources = [];
      for (let part = 0; part < segments; part++) {
        const bytes =
          segments === 1
            ? base
            : api.buildIndex(docs.filter(({ key }) => key % segments === part));
        sources.push({ source: bytes, expected: await descriptor(bytes) });
      }
      const candidate = api.AsearchIndex.fromStrings([], indexOptions);
      try {
        const memoryBeforeLoad = memoryBytes();
        const begin = performance.now();
        await candidate.replaceSegments(sources);
        const loadMs = performance.now() - begin;
        const memoryAfterLoad = memoryBytes();
        const pairedRuns = [];
        function execute(index, raw) {
          inputs.forEach((pattern, i) => {
            const start = performance.now();
            const result = index.search(pattern, searchOptions);
            const elapsed = performance.now() - start;
            if (!Number.isFinite(elapsed) || elapsed < 0)
              throw new Error("Invalid timer");
            raw?.push(elapsed);
            if (fingerprint(result) !== expected[i])
              throw new Error(
                `Segment result mismatch (${segments}, ${pattern})`,
              );
          });
        }
        for (let i = 0; i < warmup; i++) {
          execute(baseline);
          execute(candidate);
        }
        for (let run = 0; run < runs; run++) {
          const full = [],
            column = [];
          for (let round = 0; round < rounds; round++) {
            if ((run + round) % 2 === 0) {
              execute(baseline, full);
              execute(candidate, column);
            } else {
              execute(candidate, column);
              execute(baseline, full);
            }
          }
          pairedRuns.push({ baseline: full, candidate: column });
        }
        const start = performance.now(),
          compact = candidate.toBytes(),
          compactMs = performance.now() - start;
        if (!sameBytes(compact, base))
          throw new Error("Compaction bytes mismatch");
        const delta = api.buildDelta(
          [{ key: documents + 1, texts: ["new question"] }],
          [0],
        );
        const deltaExpected = await descriptor(delta);
        const memoryAfterCompact = memoryBytes();
        const setup = {
          segments,
          sources,
          base,
          baseExpected,
          delta,
          deltaExpected,
          indexOptions,
          documents,
        };
        const update = performance.now();
        await updateColumn(candidate, setup, compact);
        const updateMs = performance.now() - update;
        const comparisonSourceBytes =
          base.length +
          sources.reduce((n, source) => n + source.source.length, 0);
        const memoryAfterUpdate = memoryBytes();
        candidate.free();
        const lifecycle = await measureLifecycle(api, setup, lifecycleRepeats);
        reports.push({
          segments,
          loadMs,
          compactMs,
          updateMs,
          updateKind:
            segments < 8 ? "append" : "replace compacted base plus delta",
          retainedInputBytes: sources.reduce(
            (n, entry) => n + entry.source.length,
            0,
          ),
          comparisonSourceBytes,
          wasmMemoryByteLength: {
            beforeLoad: memoryBeforeLoad,
            afterLoad: memoryAfterLoad,
            afterCompact: memoryAfterCompact,
            afterUpdate: memoryAfterUpdate,
            afterLifecycle: memoryBytes(),
          },
          lifecycle: {
            ...lifecycle,
            load: summary(lifecycle.raw.load),
            compact: summary(lifecycle.raw.compact),
            update: summary(lifecycle.raw.update),
          },
          pairedRuns,
          baseline: summary(pairedRuns.flatMap((run) => run.baseline)),
          candidate: summary(pairedRuns.flatMap((run) => run.candidate)),
        });
      } finally {
        candidate.free();
      }
    }
    return {
      corpus: {
        kind: "synthetic deterministic question and alias strings",
        documents,
        texts: documents * 2,
        baseBytes: base.length,
        baseIntegrity: baseExpected.integrity,
      },
      options: {
        runs,
        rounds,
        warmup,
        lifecycleRepeats,
        inputs,
        ...searchOptions,
      },
      correctness:
        "ids/levels/orders/exact totals verified outside timers; compact bytes equal fresh base",
      caveat:
        "Lifecycle distributions use fresh roots, three warmups and the recorded repetitions. The first load/compact/update fields are diagnostic observations. Transfers use in-memory bytes, not HTTP network latency. Lifecycle samples are not paired engine comparisons. Timers may quantize to zero. Source byte counts describe serialized payload, not peak heap memory. WASM page sizes are non-shrinking high-water observations for the whole instance, including prior scenarios; they are not live allocation counts. No relevance or device guarantee.",
      reports,
    };
  } finally {
    baseline.free();
  }
}
