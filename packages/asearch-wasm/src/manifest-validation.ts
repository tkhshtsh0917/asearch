import { fail } from "./index-validation.js";
import type { Manifest, ManifestSegment } from "./manifest-types.js";

function invalid(message: string): never {
  return fail("INVALID_MANIFEST", message);
}
function fields(
  value: unknown,
  allowed: readonly string[],
): Record<string, unknown> {
  if (
    typeof value !== "object" ||
    value === null ||
    Array.isArray(value) ||
    Reflect.ownKeys(value).some(
      (key) => typeof key !== "string" || !allowed.includes(key),
    )
  )
    invalid("Invalid manifest object fields");
  return value as Record<string, unknown>;
}
function integer(value: unknown, min: number, max: number): number {
  if (
    typeof value !== "number" ||
    !Number.isSafeInteger(value) ||
    value < min ||
    value > max
  )
    invalid("Invalid manifest integer");
  return value;
}
function string(value: unknown, pattern: RegExp): string {
  if (typeof value !== "string" || !pattern.test(value))
    invalid("Invalid manifest string");
  return value;
}
function unsafeUrl(value: string): boolean {
  for (let i = 0; i < value.length; i++) {
    const code = value.charCodeAt(i);
    if (code <= 32 || code === 127 || code === 92) return true;
  }
  return /\s/u.test(value);
}
export function segmentSnapshot(value: unknown): ManifestSegment {
  const source = fields(value, ["id", "url", "bytes", "texts", "integrity"]);
  const id = string(source.id, /^[0-9a-f]{16}$/);
  const url = source.url;
  if (typeof url !== "string" || url.length === 0 || unsafeUrl(url))
    invalid("Invalid segment URL");
  let parsed: URL;
  try {
    parsed = new URL(url, "https://asearch.invalid/");
  } catch {
    return invalid("Invalid segment URL");
  }
  if (
    !["https:", "http:"].includes(parsed.protocol) ||
    parsed.username ||
    parsed.password
  )
    invalid("Segment URL requires HTTP(S) without userinfo");
  return {
    id,
    url,
    bytes: integer(source.bytes, 64, 0xffffffff),
    texts: integer(source.texts, 0, 0xffffffff),
    integrity: string(source.integrity, /^sha384-[A-Za-z0-9+/]{64}$/),
  };
}

/** Snapshot known fields once. No input object or array is returned to callers. */
export function manifestSnapshot(value: unknown): Manifest {
  const source = fields(value, [
    "format",
    "indexId",
    "generation",
    "index",
    "segments",
  ]);
  if (source.format !== "asearch-manifest/1")
    invalid("Unsupported manifest format");
  const indexId = string(source.indexId, /^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/);
  const generation = integer(source.generation, 1, Number.MAX_SAFE_INTEGER);
  const definition = fields(source.index, [
    "major",
    "minor",
    "symbol_unit",
    "normalizer",
  ]);
  const major = integer(definition.major, 1, 1);
  const minor = integer(definition.minor, 0, 65535);
  const unit = definition.symbol_unit;
  if (unit !== "code-point" && unit !== "utf16") invalid("Invalid symbol unit");
  const normalizer = fields(definition.normalizer, [
    "id",
    "version",
    "options",
  ]);
  const id = normalizer.id;
  if (id !== "none" && id !== "ja-basic") invalid("Unsupported normalizer");
  const version = integer(normalizer.version, 1, 1);
  const options = integer(normalizer.options, 0, id === "none" ? 0 : 63);
  const input = source.segments;
  if (!Array.isArray(input)) invalid("segments must be an array");
  const length = integer(input.length, 1, 8);
  const segments: ManifestSegment[] = [];
  const ids = new Set<string>();
  for (let i = 0; i < length; i++) {
    const segment = segmentSnapshot(input[i]);
    if (ids.has(segment.id)) invalid("Duplicate segment ID");
    ids.add(segment.id);
    segments.push(segment);
  }
  return {
    format: "asearch-manifest/1",
    indexId,
    generation,
    index: {
      major,
      minor,
      symbol_unit: unit,
      normalizer: { id, version, options },
    },
    segments,
  };
}
