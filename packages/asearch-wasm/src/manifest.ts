import { fail } from "./index-validation.js";
import type { Manifest, ManifestDiff } from "./manifest-types.js";
import { manifestSnapshot } from "./manifest-validation.js";

export type {
  Manifest,
  ManifestDiff,
  ManifestSegment,
} from "./manifest-types.js";

/** Pure comparison of fully applied manifests. Fetching and storing the new
 * current manifest belongs to the caller after atomic index adoption. */
export function diffManifest(
  current: Manifest | undefined,
  next: Manifest,
): ManifestDiff {
  const candidate = manifestSnapshot(next);
  const active = current === undefined ? undefined : manifestSnapshot(current);
  if (!active) return { kind: "replace", segments: candidate.segments };
  if (
    active.indexId !== candidate.indexId ||
    JSON.stringify(active.index) !== JSON.stringify(candidate.index)
  )
    fail(
      "CONFLICT",
      "Manifest belongs to a different index series or definition",
    );
  if (candidate.generation < active.generation) return { kind: "stale" };
  if (candidate.generation === active.generation) {
    if (JSON.stringify(active) !== JSON.stringify(candidate))
      fail("CONFLICT", "Manifest changed within the same generation");
    return { kind: "none" };
  }
  const known = new Map(
    active.segments.map((segment) => [segment.id, segment]),
  );
  for (const segment of candidate.segments) {
    const old = known.get(segment.id);
    if (
      old &&
      (old.bytes !== segment.bytes ||
        old.texts !== segment.texts ||
        old.integrity !== segment.integrity)
    )
      fail("CONFLICT", "Known segment content descriptor changed");
  }
  const prefix =
    active.segments.length <= candidate.segments.length &&
    active.segments.every(
      (segment, i) => segment.id === candidate.segments[i].id,
    );
  if (!prefix) return { kind: "replace", segments: candidate.segments };
  const add = candidate.segments.slice(active.segments.length);
  return add.length ? { kind: "append", add } : { kind: "none" };
}
