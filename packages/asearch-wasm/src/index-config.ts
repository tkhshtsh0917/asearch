import { expectedWire } from "./index-expected.js";
import {
  type AmbigCallback,
  type AmbigDescriptor,
  type AmbigPolicy,
  byLength,
  fixed,
  type IndexOptions,
  type LoadOptions,
  type SearchOptions,
} from "./index-options.js";
import {
  boolean,
  checkKeys,
  fail,
  integer,
  optional,
} from "./index-validation.js";
import { MatchMode } from "./pattern.js";
export function loadConfig(options: LoadOptions = {}): {
  config: Uint32Array;
  maxBytes: number;
  expected: Uint8Array;
} {
  checkKeys(
    options,
    ["profile", "normalizer", "blockWords", "limits", "maxBytes", "expected"],
    "load options",
  );
  const maxBytes = optional(options.maxBytes, 256 * 1024 * 1024);
  const indexOptions: IndexOptions = {
    profile: options.profile,
    normalizer: options.normalizer,
    blockWords: options.blockWords,
    limits: options.limits,
  };
  return {
    config: indexConfig(indexOptions),
    maxBytes: integer(maxBytes, "maxBytes", 1),
    expected: expectedWire(options.expected),
  };
}

export function indexConfig(options: IndexOptions = {}): Uint32Array {
  checkKeys(
    options,
    ["profile", "normalizer", "blockWords", "limits"],
    "index options",
  );
  const profiles = { "asearch.ts": 0, "asearch.ts-lib": 1, ideal: 2 };
  const name = optional(options.profile, "ideal");
  if (typeof name !== "string" || !Object.hasOwn(profiles, name))
    fail("INVALID_INPUT", "Unknown profile");
  const n = optional(options.normalizer, "ja-basic-v1");
  const normalizer =
    n === "none"
      ? { id: "none", version: 1, options: 0 }
      : n === "ja-basic-v1"
        ? { id: "ja-basic", version: 1, options: 31 }
        : n;
  if (typeof normalizer !== "object" || normalizer === null)
    fail("INVALID_INPUT", "Unknown normalizer");
  const l = optional(options.limits, {});
  checkKeys(
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
  checkKeys(normalizer, ["id", "version", "options"], "normalizer");
  const { id, version: rawVersion, options: rawFlags } = normalizer;
  if (id !== "none" && id !== "ja-basic")
    fail("INVALID_INPUT", "Unknown normalizer");
  const version = integer(rawVersion, "normalizer version", 1, 65535);
  const flags = integer(rawFlags, "normalizer options", 0, 63);
  if (version !== 1)
    fail("UNSUPPORTED_FORMAT", "Unsupported normalizer version");
  if (id === "none" && flags !== 0)
    fail("INVALID_INPUT", "Unsupported normalization descriptor");
  const values = [
    profiles[name],
    id === "none" ? 0 : 1,
    version,
    flags,
    integer(optional(options.blockWords, 512), "blockWords", 1, 0xffffffff),
    integer(optional(l.maxTotalBytes, 256 * 1024 * 1024), "maxTotalBytes", 1),
    integer(optional(l.maxTexts, 1000000), "maxTexts", 1),
    integer(optional(l.maxUnits, 10000000), "maxUnits", 1),
    integer(optional(l.maxSlots, 16777216), "maxSlots", 1),
    integer(
      optional(l.maxWorkingBytes, 512 * 1024 * 1024),
      "maxWorkingBytes",
      1,
    ),
    integer(optional(l.maxPatternUnits, 65535), "maxPatternUnits", 1),
    integer(optional(l.maxSegments, 8), "maxSegments", 1, 8),
  ];
  if (values[9] < 128)
    fail("RESOURCE_LIMIT", "Working budget cannot hold boundary metadata");
  // A logical upper bound above the wasm32 address space can be saturated;
  // it must never wrap to zero while crossing the FFI.
  return Uint32Array.from(values, (v) => Math.min(v, 0xffffffff));
}

export function searchConfig(options: SearchOptions = {}): {
  config: Uint32Array;
  limit: number;
  countTotal: boolean;
  callback?: AmbigCallback;
} {
  checkKeys(
    options,
    ["mode", "wrap", "maxAmbig", "limit", "countTotal", "engine"],
    "search options",
  );
  const mode = integer(
    optional(options.mode, MatchMode.WildcardSpace),
    "mode",
    0,
    2,
  );
  const wrap = boolean(optional(options.wrap, true), "wrap");
  const sourceOption = options.maxAmbig;
  const source = sourceOption === undefined ? byLength() : sourceOption;
  const callback = typeof source === "function" ? source : undefined;
  const policy = callback ? fixed(0) : parsePolicy(source);
  const engine = optional(options.engine, "auto");
  if (typeof engine !== "string") fail("INVALID_INPUT", "Invalid engine");
  if (engine !== "auto" && engine !== "transposed-blocked")
    fail("UNSUPPORTED_OPTION", "Unsupported engine");
  const limit = Math.min(
    integer(optional(options.limit, 30), "limit"),
    0xffffffff,
  );
  const countTotal = boolean(optional(options.countTotal, false), "countTotal");
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
  return { config: Uint32Array.from(config), limit, countTotal, callback };
}

function parsePolicy(source: number | AmbigPolicy): AmbigDescriptor {
  if (typeof source === "number") return fixed(source);
  if (typeof source !== "object" || source === null)
    fail("INVALID_INPUT", "Unknown ambiguity policy");
  const kind = source.kind;
  if (kind === "fixed") {
    checkKeys(source, ["kind", "value"], "fixed policy");
    return fixed(source.value);
  }
  if (kind === "by-length") {
    checkKeys(
      source,
      ["kind", "every", "max", "unit", "includeWildcards"],
      "byLength policy",
    );
    return byLength({
      every: integer(source.every, "every", 1),
      max: integer(source.max, "max", 0, 7),
      unit: source.unit,
      includeWildcards: source.includeWildcards,
    });
  }
  return fail("INVALID_INPUT", "Unknown ambiguity policy");
}
