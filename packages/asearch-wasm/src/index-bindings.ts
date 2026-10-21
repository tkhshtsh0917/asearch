import type { UpdateHandle } from "./index-update.js";
export interface IndexHandle {
  check_transport(bytes: number): void;
  free(): void;
  clear_workspace(): void;
  begin_update(append: boolean): UpdateHandle;
  segments_packed(): Uint32Array;
  explain_prepared(
    ref: number,
    keys: Uint32Array,
    original: Uint16Array,
    supplied: boolean,
  ): Uint32Array;
  to_bytes(): Uint8Array;
  fork_session(maxCacheBytes: number, maxAmbig: number): IndexHandle;
  reset_session(): void;
  select_level(level: number): void;
  prepare_u16(units: Uint16Array, config: Uint32Array): Uint16Array;
  search_prepared(
    keys: Uint32Array,
    limit: number,
    countTotal: boolean,
  ): Uint32Array;
}
export interface IndexConstructor {
  from_flat(
    units: Uint16Array,
    lengths: Uint32Array,
    keys: Uint32Array,
    orders: Uint32Array,
    config: Uint32Array,
  ): IndexHandle;
  build_delta_flat(
    units: Uint16Array,
    lengths: Uint32Array,
    keys: Uint32Array,
    orders: Uint32Array,
    tombstones: Uint32Array,
    config: Uint32Array,
  ): Uint8Array;
  from_bytes(
    bytes: Uint8Array,
    config: Uint32Array,
    expected: Uint8Array,
  ): IndexHandle;
}

let binding: IndexConstructor | undefined;
export function installIndexConstructor(value: IndexConstructor): void {
  binding = value;
}

export function requireBinding(): IndexConstructor {
  if (!binding)
    throw new Error("asearch-wasm is not initialized; await init() first");
  return binding;
}
