import { indexConfig, searchConfig } from "./index-config.js";
import { type Explanation, explainSearch } from "./index-explain.js";
import { flatten } from "./index-input.js";
import type { IndexOptions, SearchOptions } from "./index-options.js";
import type { SearchResult } from "./index-result.js";
import {
  nativeCall as call,
  executeSearch,
  type SearchHandle,
} from "./index-search.js";
import {
  checkKeys,
  fail,
  inputTypeError,
  integer,
  optional,
} from "./index-validation.js";

export interface IntentEntry {
  expr: string;
  key: number;
  order?: number;
}
export type IntentSearchOptions = Omit<SearchOptions, "engine"> & {
  engine?: "auto" | "dag";
};
interface Handle extends SearchHandle {
  free(): void;
  explain_prepared(
    ref: number,
    keys: Uint32Array,
    original: Uint16Array,
    supplied: boolean,
  ): Uint32Array;
}
interface Constructor {
  from_flat(
    units: Uint16Array,
    lengths: Uint32Array,
    keys: Uint32Array,
    orders: Uint32Array,
    config: Uint32Array,
  ): Handle;
}
let binding: Constructor | undefined;
export function installIntentConstructor(value: Constructor): void {
  binding = value;
}
interface State {
  handle: Handle;
  maxUnits: number;
  profile: "ideal" | "asearch.ts" | "asearch.ts-lib";
  busy: boolean;
}
const states = new WeakMap<IntentIndex, State | null>();
const finalizer = new FinalizationRegistry<Handle>((handle) => handle.free());
const secret = Symbol("IntentIndex factory");
function state(receiver: IntentIndex): State {
  const value = states.get(receiver);
  if (value === null) fail("DISPOSED", "IntentIndex has been disposed");
  if (!value) throw inputTypeError("Invalid IntentIndex receiver");
  return value;
}
function optionsSnapshot(options: IntentSearchOptions = {}): SearchOptions {
  checkKeys(
    options,
    ["mode", "wrap", "maxAmbig", "limit", "countTotal", "engine"],
    "intent search options",
  );
  const { mode, wrap, maxAmbig, limit, countTotal, engine } = options;
  if (!["auto", "dag"].includes(optional(engine, "auto")))
    fail("UNSUPPORTED_OPTION", "IntentIndex requires the DAG engine");
  return { mode, wrap, maxAmbig, limit, countTotal, engine: "auto" };
}
/** Immutable bounded intent expressions. Choices are searched without expansion. */
export class IntentIndex {
  private constructor(handle: Handle, config: Uint32Array, token: symbol) {
    if (token !== secret) throw inputTypeError("Use IntentIndex.fromEntries()");
    states.set(this, {
      handle,
      maxUnits: config[10],
      profile:
        config[0] === 0
          ? "asearch.ts"
          : config[0] === 1
            ? "asearch.ts-lib"
            : "ideal",
      busy: false,
    });
    finalizer.register(this, handle, this);
  }
  static fromEntries(
    entries: readonly IntentEntry[],
    options?: IndexOptions,
  ): IntentIndex {
    if (!binding)
      throw new Error("asearch-wasm is not initialized; await init() first");
    const factory = binding,
      config = indexConfig(options);
    if (!Array.isArray(entries))
      throw inputTypeError("entries must be an array");
    const count = integer(entries.length, "entries.length", 0, config[6]);
    function* documents() {
      for (let i = 0; i < count; i++) {
        const entry = entries[i];
        checkKeys(entry, ["key", "order", "expr"], "intent entry");
        const { key, order, expr } = entry;
        if (typeof expr !== "string")
          throw inputTypeError("expr must be a primitive string");
        yield { key, order, texts: [expr] };
      }
    }
    const flat = flatten(documents(), config);
    return new IntentIndex(
      call(() =>
        factory.from_flat(
          flat.units,
          flat.lengths,
          flat.keys,
          flat.orders,
          config,
        ),
      ),
      config,
      secret,
    );
  }
  search(pattern: string, options?: IntentSearchOptions): SearchResult {
    const value = state(this);
    if (value.busy)
      fail("CONFLICT", "IntentIndex search is already in progress");
    value.busy = true;
    try {
      const prepared = searchConfig(optionsSnapshot(options));
      return executeSearch(
        () => state(this).handle,
        pattern,
        prepared,
        value.maxUnits,
        value.profile,
      );
    } finally {
      value.busy = false;
    }
  }
  explain(
    reference: number,
    pattern: string,
    options?: IntentSearchOptions,
  ): Explanation | null {
    const value = state(this);
    if (value.busy)
      fail("CONFLICT", "IntentIndex search is already in progress");
    value.busy = true;
    try {
      return explainSearch(
        () => state(this).handle,
        reference,
        pattern,
        optionsSnapshot(options),
        value.maxUnits,
        value.profile,
      );
    } finally {
      value.busy = false;
    }
  }
  free(): void {
    const value = states.get(this);
    if (value === undefined)
      throw inputTypeError("Invalid IntentIndex receiver");
    if (value === null) return;
    states.set(this, null);
    finalizer.unregister(this);
    value.handle.free();
  }
  declare [Symbol.dispose]: () => void;
}
Object.defineProperty(
  IntentIndex.prototype,
  Symbol.dispose ?? Symbol.for("Symbol.dispose"),
  { value: IntentIndex.prototype.free, configurable: true, writable: true },
);
