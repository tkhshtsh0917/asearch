export function sameBytes(left, right) {
  if (left.length !== right.length) return false;
  for (let i = 0; i < left.length; i++) if (left[i] !== right[i]) return false;
  return true;
}
export async function updateColumn(
  index,
  { segments, delta, deltaExpected, baseExpected },
  compact,
) {
  if (segments < 8) await index.addSegment(delta, deltaExpected);
  else
    await index.replaceSegments([
      { source: compact, expected: baseExpected },
      { source: delta, expected: deltaExpected },
    ]);
}
function verifyUpdate(index, documents) {
  const options = { mode: 0, maxAmbig: 0 };
  const added = index.search("new question", options);
  if (added.length !== 1 || added.ids[0] !== documents + 1)
    throw new Error("Update failed to add the synthetic document");
  if (index.search("質問 0 日本の歴史と地理の使い方", options).length !== 0)
    throw new Error("Update failed to remove the old key");
}
export async function measureLifecycle(api, setup, repetitions) {
  const raw = { load: [], compact: [], update: [] };
  const warmup = 3;
  for (let sample = 0; sample < warmup + repetitions; sample++) {
    const index = api.AsearchIndex.fromStrings([], setup.indexOptions);
    try {
      const loadStart = performance.now();
      await index.replaceSegments(setup.sources);
      const load = performance.now() - loadStart;
      const compactStart = performance.now();
      const bytes = index.toBytes();
      const compact = performance.now() - compactStart;
      if (!sameBytes(bytes, setup.base))
        throw new Error("Lifecycle compaction bytes mismatch");
      const updateStart = performance.now();
      await updateColumn(index, setup, bytes);
      const update = performance.now() - updateStart;
      verifyUpdate(index, setup.documents);
      if (sample >= warmup) {
        raw.load.push(load);
        raw.compact.push(compact);
        raw.update.push(update);
      }
    } finally {
      index.free();
    }
  }
  if (
    Object.values(raw)
      .flat()
      .some((value) => !Number.isFinite(value) || value < 0)
  )
    throw new Error("Invalid lifecycle timer");
  return { warmup, repetitions, raw };
}
