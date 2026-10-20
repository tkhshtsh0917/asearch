import type { ManifestSegment } from "./manifest-types.js";
import { segmentSnapshot } from "./manifest-validation.js";

/** Fixed descriptor: xxh3 u64 LE, length/count u32 LE, raw SHA-384 digest. */
export function expectedWire(value: ManifestSegment | undefined): Uint8Array {
  if (value === undefined) return new Uint8Array();
  const segment = segmentSnapshot(value);
  const wire = new Uint8Array(64);
  const view = new DataView(wire.buffer);
  view.setBigUint64(0, BigInt(`0x${segment.id}`), true);
  view.setUint32(8, segment.bytes, true);
  view.setUint32(12, segment.texts, true);
  const digest = atob(segment.integrity.slice(7));
  for (let i = 0; i < 48; i++) wire[16 + i] = digest.charCodeAt(i);
  return wire;
}
