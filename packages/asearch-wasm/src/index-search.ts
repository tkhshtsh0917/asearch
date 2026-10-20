import { casingKeys, fromUtf16 } from "./index-casing.js";
import type { searchConfig } from "./index-config.js";
import { utf16 } from "./index-input.js";
import { type SearchResult, unpack } from "./index-result.js";
import { fail, integer } from "./index-validation.js";

export interface SearchHandle {
  select_level(level: number): void;
  prepare_u16(units: Uint16Array, config: Uint32Array): Uint16Array;
  search_prepared(
    keys: Uint32Array,
    limit: number,
    countTotal: boolean,
  ): Uint32Array;
}
export function nativeCall<T>(operation: () => T): T {
  try {
    return operation();
  } catch (error) {
    if (error instanceof Error) {
      const match =
        /^(INVALID_INPUT|INVALID_INDEX|RESOURCE_LIMIT): (.*)$/s.exec(
          error.message,
        );
      if (match)
        throw Object.assign(new Error(match[2], { cause: error }), {
          code: match[1],
        });
    }
    throw error;
  }
}

export function executeSearch(
  handle: () => SearchHandle,
  pattern: string,
  prepared: ReturnType<typeof searchConfig>,
  maxUnits: number,
  profile: "ideal" | "asearch.ts" | "asearch.ts-lib",
): SearchResult {
  handle();
  if (typeof pattern !== "string")
    throw new TypeError("Expected a primitive string");
  if (pattern.length > maxUnits)
    fail("RESOURCE_LIMIT", "Query exceeds UTF-16 unit limit");
  const units = utf16(pattern);
  const patternUnits = nativeCall(() =>
    handle().prepare_u16(units, prepared.config),
  );
  if (prepared.callback) {
    const normalized = fromUtf16(
      prepared.config[0] === 2 && prepared.config[1] === 1
        ? patternUnits.subarray(1, patternUnits.length - 1)
        : patternUnits,
    );
    const level = integer(
      prepared.callback(
        Object.freeze({ normalized, mode: prepared.config[0], profile }),
      ),
      "custom ambiguity",
      0,
      7,
    );
    nativeCall(() => handle().select_level(level));
  }
  const keys = casingKeys(patternUnits, prepared.config[0]);
  return unpack(
    nativeCall(() =>
      handle().search_prepared(keys, prepared.limit, prepared.countTotal),
    ),
  );
}
