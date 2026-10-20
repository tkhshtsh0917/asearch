import { MatchMode } from "./pattern.js";

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
  profile?: "ideal";
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
}

export function loadConfig(options: LoadOptions = {}): {
  config: Uint32Array;
  maxBytes: number;
} {
  keys(
    options,
    ["profile", "normalizer", "blockWords", "limits", "maxBytes"],
    "load options",
  );
  const { maxBytes = 256 * 1024 * 1024, ...indexOptions } = options;
  return {
    config: indexConfig(indexOptions),
    maxBytes: integer(maxBytes, "maxBytes", 1),
  };
}
export type AmbigPolicy =
  | { kind: "fixed"; value: number }
  | {
      kind: "by-length";
      every: number;
      max: number;
      unit?: "profile" | "code-point" | "utf16";
      includeWildcards?: boolean;
    };
export interface SearchOptions {
  mode?: MatchMode;
  wrap?: boolean;
  maxAmbig?: number | AmbigPolicy;
  limit?: number;
  countTotal?: boolean;
  engine?: "auto" | "transposed-blocked";
}

export function fail(code: string, message: string): never {
  throw Object.assign(new Error(message), { code });
}

function keys(value: unknown, allowed: readonly string[], name: string): void {
  if (typeof value !== "object" || value === null)
    throw new TypeError(`${name} must be an object`);
  if (Object.keys(value).some((key) => !allowed.includes(key)))
    fail("INVALID_INPUT", `Unknown ${name} field`);
}

export function integer(
  value: unknown,
  name: string,
  min = 0,
  max = Number.MAX_SAFE_INTEGER,
): number {
  if (typeof value !== "number")
    throw new TypeError(`${name} must be a number`);
  if (!Number.isSafeInteger(value) || value < min || value > max)
    fail("INVALID_INPUT", `${name} must be an integer in ${min}..${max}`);
  return value;
}
function boolean(value: unknown, name: string): boolean {
  if (typeof value !== "boolean")
    throw new TypeError(`${name} must be a boolean`);
  return value;
}

export function fixed(k: number): AmbigPolicy {
  return Object.freeze({ kind: "fixed", value: integer(k, "K", 0, 7) });
}
export function byLength(
  options: {
    every?: number;
    max?: number;
    unit?: "profile" | "code-point" | "utf16";
    includeWildcards?: boolean;
  } = {},
): AmbigPolicy {
  keys(options, ["every", "max", "unit", "includeWildcards"], "byLength");
  const every = integer(options.every ?? 4, "every", 1);
  const max = integer(options.max ?? 3, "max", 0, 7);
  const unit = options.unit ?? "profile";
  if (!["profile", "code-point", "utf16"].includes(unit))
    fail("INVALID_INPUT", "Unknown length unit");
  const includeWildcards = boolean(
    options.includeWildcards ?? false,
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

export function indexConfig(options: IndexOptions = {}): Uint32Array {
  keys(
    options,
    ["profile", "normalizer", "blockWords", "limits"],
    "index options",
  );
  const profiles = { ideal: 2 };
  const name = options.profile ?? "ideal";
  if (!Object.hasOwn(profiles, name)) fail("INVALID_INPUT", "Unknown profile");
  const n = options.normalizer ?? "ja-basic-v1";
  const normalizer =
    n === "none"
      ? { id: "none", version: 1, options: 0 }
      : n === "ja-basic-v1"
        ? { id: "ja-basic", version: 1, options: 31 }
        : n;
  if (
    typeof normalizer !== "object" ||
    normalizer === null ||
    !["none", "ja-basic"].includes(normalizer.id)
  )
    fail("INVALID_INPUT", "Unknown normalizer");
  const l = options.limits ?? {};
  keys(
    l,
    [
      "maxTotalBytes",
      "maxTexts",
      "maxUnits",
      "maxSlots",
      "maxWorkingBytes",
      "maxPatternUnits",
      "maxSegments",
    ],
    "limits",
  );
  keys(normalizer, ["id", "version", "options"], "normalizer");
  if (
    normalizer.version !== 1 ||
    (normalizer.id === "none" && normalizer.options !== 0)
  )
    fail("INVALID_INPUT", "Unsupported normalization descriptor");
  const values = [
    profiles[name],
    normalizer.id === "none" ? 0 : 1,
    integer(normalizer.version, "normalizer version", 1, 65535),
    integer(normalizer.options, "normalizer options", 0, 63),
    integer(options.blockWords ?? 512, "blockWords", 1, 0xffffffff),
    integer(l.maxTotalBytes ?? 256 * 1024 * 1024, "maxTotalBytes", 1),
    integer(l.maxTexts ?? 1000000, "maxTexts", 1),
    integer(l.maxUnits ?? 10000000, "maxUnits", 1),
    integer(l.maxSlots ?? 16777216, "maxSlots", 1),
    integer(l.maxWorkingBytes ?? 512 * 1024 * 1024, "maxWorkingBytes", 1),
    integer(l.maxPatternUnits ?? 65535, "maxPatternUnits", 1),
    integer(l.maxSegments ?? 8, "maxSegments", 1, 8),
  ];
  // A logical upper bound above the wasm32 address space can be saturated;
  // it must never wrap to zero while crossing the FFI.
  return Uint32Array.from(values, (v) => Math.min(v, 0xffffffff));
}

export function searchConfig(options: SearchOptions = {}): {
  config: Uint32Array;
  limit: number;
  countTotal: boolean;
} {
  keys(
    options,
    ["mode", "wrap", "maxAmbig", "limit", "countTotal", "engine"],
    "search options",
  );
  const mode = integer(options.mode ?? MatchMode.WildcardSpace, "mode", 0, 2);
  const wrap = boolean(options.wrap ?? true, "wrap");
  const source = options.maxAmbig ?? byLength();
  let policy: AmbigPolicy;
  if (typeof source === "number") policy = fixed(source);
  else if (source?.kind === "fixed") {
    keys(source, ["kind", "value"], "fixed policy");
    policy = fixed(source.value);
  } else if (source?.kind === "by-length") {
    keys(
      source,
      ["kind", "every", "max", "unit", "includeWildcards"],
      "byLength policy",
    );
    policy = byLength({
      every: source.every,
      max: source.max,
      unit: source.unit,
      includeWildcards: source.includeWildcards,
    });
  } else fail("INVALID_INPUT", "Unknown ambiguity policy");
  const engine = options.engine ?? "auto";
  if (engine !== "auto" && engine !== "transposed-blocked")
    fail("INVALID_INPUT", "Unsupported engine");
  const limit = Math.min(integer(options.limit ?? 30, "limit"), 0xffffffff);
  const countTotal = boolean(options.countTotal ?? false, "countTotal");
  const config =
    policy.kind === "fixed"
      ? [mode, +wrap, 0, policy.value, 1, 0, 0]
      : [
          mode,
          +wrap,
          1,
          policy.max,
          Math.min(policy.every, 0xffffffff),
          policy.unit === "utf16" ? 2 : policy.unit === "code-point" ? 1 : 0,
          +(policy.includeWildcards ?? false),
        ];
  return { config: Uint32Array.from(config), limit, countTotal };
}
