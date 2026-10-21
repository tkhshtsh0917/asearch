import { fromUtf16 } from "./index-casing.js";
import { searchConfig } from "./index-config.js";
import { utf16 } from "./index-input.js";
import type { SearchOptions } from "./index-options.js";
import {
  nativeCall as call,
  prepareSearch,
  type SearchHandle,
} from "./index-search.js";
import {
  checkKeys,
  fail,
  inputTypeError,
  integer,
} from "./index-validation.js";

export interface Explanation {
  text: string;
  level: number;
  spans: [number, number][];
  coordinateSpace: "original-utf16" | "normalized-utf16";
}
interface ExplainHandle extends SearchHandle {
  explain_prepared(
    ref: number,
    keys: Uint32Array,
    original: Uint16Array,
    supplied: boolean,
  ): Uint32Array;
}
export function explainSearch(
  handle: () => ExplainHandle,
  reference: number,
  pattern: string,
  options: SearchOptions & { original?: string },
  maxUnits: number,
  profile: "ideal" | "asearch.ts" | "asearch.ts-lib",
): Explanation | null {
  const ref = integer(reference, "reference", 0, 0xffffffff);
  checkKeys(
    options,
    ["mode", "wrap", "maxAmbig", "limit", "countTotal", "engine", "original"],
    "explain options",
  );
  const { mode, wrap, maxAmbig, limit, countTotal, engine, original } = options;
  if (original !== undefined && typeof original !== "string")
    throw inputTypeError("original must be a primitive string");
  if (original !== undefined && original.length > 65535)
    fail("RESOURCE_LIMIT", "Original text exceeds UTF-16 unit limit");
  const prepared = searchConfig({
    mode,
    wrap,
    maxAmbig,
    limit,
    countTotal,
    engine,
  });
  const keys = prepareSearch(handle, pattern, prepared, maxUnits, profile);
  call(() =>
    handle().check_transport(keys.byteLength + (original?.length ?? 0) * 2),
  );
  const packed = call(() =>
    handle().explain_prepared(
      ref,
      keys,
      utf16(original ?? ""),
      original !== undefined,
    ),
  );
  if (packed.length === 0) return null;
  const spans: [number, number][] = [];
  for (let i = 0; i < packed[3]; i++)
    spans.push([packed[4 + packed[2] + i * 2], packed[5 + packed[2] + i * 2]]);
  return {
    text: fromUtf16(new Uint16Array(packed.subarray(4, 4 + packed[2]))),
    level: packed[0],
    spans,
    coordinateSpace: packed[1] === 1 ? "original-utf16" : "normalized-utf16",
  };
}
