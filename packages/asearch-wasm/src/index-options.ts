import {
  boolean,
  checkKeys,
  fail,
  integer,
  optional,
} from "./index-validation.js";
import type { ManifestSegment } from "./manifest-types.js";
import type { MatchMode } from "./pattern.js";

export interface ResourceLimits {
  maxTotalBytes?: number;
  maxTexts?: number;
  maxUnits?: number;
  maxSlots?: number;
  maxWorkingBytes?: number;
  maxPatternUnits?: number;
  maxSegments?: number;
}
export interface NormalizerConfig {
  id: "none" | "ja-basic";
  version: number;
  options: number;
}
export interface IndexOptions {
  profile?: "asearch.ts" | "asearch.ts-lib" | "ideal";
  normalizer?: "none" | "ja-basic-v1" | NormalizerConfig;
  blockWords?: number;
  limits?: ResourceLimits;
}
export interface Doc {
  key: number;
  order?: number;
  texts: readonly string[];
}
export interface LoadOptions extends IndexOptions {
  maxBytes?: number;
  expected?: ManifestSegment;
}

export interface AmbigContext {
  normalized: string;
  mode: MatchMode;
  profile: "asearch.ts" | "asearch.ts-lib" | "ideal";
}
export type AmbigCallback = (context: AmbigContext) => number;
export type AmbigDescriptor =
  | { kind: "fixed"; value: number }
  | {
      kind: "by-length";
      every: number;
      max: number;
      unit?: "profile" | "code-point" | "utf16";
      includeWildcards?: boolean;
    };
export type AmbigPolicy = AmbigDescriptor | AmbigCallback;

export interface SearchOptions {
  mode?: MatchMode;
  wrap?: boolean;
  maxAmbig?: number | AmbigPolicy;
  limit?: number;
  countTotal?: boolean;
  engine?: "auto" | "transposed-blocked";
}

export function fixed(k: number): AmbigDescriptor {
  return Object.freeze({ kind: "fixed", value: integer(k, "K", 0, 7) });
}
export function byLength(
  options: {
    every?: number;
    max?: number;
    unit?: "profile" | "code-point" | "utf16";
    includeWildcards?: boolean;
  } = {},
): AmbigDescriptor {
  checkKeys(options, ["every", "max", "unit", "includeWildcards"], "byLength");
  const every = integer(optional(options.every, 4), "every", 1);
  const max = integer(optional(options.max, 3), "max", 0, 7);
  const unit = optional(options.unit, "profile");
  if (!["profile", "code-point", "utf16"].includes(unit))
    fail("INVALID_INPUT", "Unknown length unit");
  const includeWildcards = boolean(
    optional(options.includeWildcards, false),
    "includeWildcards",
  );
  return Object.freeze({
    kind: "by-length",
    every,
    max,
    unit,
    includeWildcards,
  });
}
