import { type Explanation, explainSearch } from "./index-explain.js";

export type { Explanation } from "./index-explain.js";

import { indexConfig, loadConfig, searchConfig } from "./index-config.js";
import { expectedWire } from "./index-expected.js";
import { flatten } from "./index-input.js";
import type {
  Doc,
  IndexOptions,
  LoadOptions,
  SearchOptions,
} from "./index-options.js";
import type { SearchResult } from "./index-result.js";
import { nativeCall as call, executeSearch } from "./index-search.js";
import { SearchSession } from "./index-session.js";
import { loadStream } from "./index-stream.js";
import {
  applyUpdate,
  type UpdateHandle,
  type UpdateSource,
} from "./index-update.js";
import { checkKeys, fail, integer, optional } from "./index-validation.js";
import type { ManifestSegment } from "./manifest-types.js";

export type {
  AmbigContext,
  AmbigPolicy,
  Doc,
  IndexOptions,
  LoadOptions,
  NormalizerConfig,
  ResourceLimits,
  SearchOptions,
} from "./index-options.js";
export { byLength, fixed } from "./index-options.js";
export type { SearchResult, SearchStats } from "./index-result.js";
export { SearchSession } from "./index-session.js";

export interface IndexHandle {
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
const secret = Symbol("AsearchIndex factory");
const handles = new WeakMap<AsearchIndex, IndexHandle | null>();
const searching = new WeakSet<AsearchIndex>();
const updates = new WeakMap<AsearchIndex, AbortController>();
const activeSearches = new WeakMap<AsearchIndex, number>();
function duringSearch<T>(index: AsearchIndex, operation: () => T): T {
  activeSearches.set(index, (activeSearches.get(index) ?? 0) + 1);
  try {
    return operation();
  } finally {
    activeSearches.set(index, (activeSearches.get(index) ?? 1) - 1);
  }
}
export interface SegmentInfo {
  id: string;
  bytes: number;
  texts: number;
  dead: number;
}
export interface SegmentSource {
  source: Uint8Array | ReadableStream<Uint8Array>;
  expected?: ManifestSegment;
}

const children = new WeakMap<
  AsearchIndex,
  Set<{ free(): void; clear(): void }>
>();
const profiles = new WeakMap<
  AsearchIndex,
  "ideal" | "asearch.ts" | "asearch.ts-lib"
>();
const patternLimits = new WeakMap<AsearchIndex, number>();
const finalizer = new FinalizationRegistry<IndexHandle>((handle) =>
  handle.free(),
);

export function installIndexConstructor(value: IndexConstructor): void {
  binding = value;
}

function requireBinding(): IndexConstructor {
  if (!binding)
    throw new Error("asearch-wasm is not initialized; await init() first");
  return binding;
}
function requireHandle(receiver: AsearchIndex): IndexHandle {
  const handle = handles.get(receiver);
  if (handle === null) fail("DISPOSED", "AsearchIndex has been disposed");
  if (!handle) throw new TypeError("Invalid AsearchIndex receiver");
  return handle;
}
export class AsearchIndex {
  private constructor(
    handle: IndexHandle,
    token: symbol,
    maxPatternUnits: number,
    profile: number,
  ) {
    if (token !== secret) throw new TypeError("Use an AsearchIndex factory");
    handles.set(this, handle);
    children.set(this, new Set());
    patternLimits.set(this, maxPatternUnits);
    profiles.set(
      this,
      profile === 0 ? "asearch.ts" : profile === 1 ? "asearch.ts-lib" : "ideal",
    );
    finalizer.register(this, handle, this);
  }

  static fromDocs(docs: Iterable<Doc>, options?: IndexOptions): AsearchIndex {
    const handleFactory = requireBinding();
    const config = indexConfig(options);
    const { units, lengths, keys, orders } = flatten(docs, config);
    return new AsearchIndex(
      call(() => handleFactory.from_flat(units, lengths, keys, orders, config)),
      secret,
      config[10],
      config[0],
    );
  }

  static fromStrings(
    texts: readonly string[],
    options?: IndexOptions,
  ): AsearchIndex {
    requireBinding();
    if (!Array.isArray(texts)) throw new TypeError("texts must be an array");
    function* docs(): Iterable<Doc> {
      for (let key = 0; key < texts.length; key++)
        yield { key, texts: [texts[key]] };
    }
    return AsearchIndex.fromDocs(docs(), options);
  }

  static fromBytes(bytes: Uint8Array, options?: LoadOptions): AsearchIndex {
    const handleFactory = requireBinding();
    if (!(bytes instanceof Uint8Array))
      throw new TypeError("Expected Uint8Array");
    const { config, maxBytes, expected } = loadConfig(options);
    if (bytes.length > Math.min(config[5], maxBytes))
      fail("RESOURCE_LIMIT", "Index input exceeds byte limit");
    return new AsearchIndex(
      call(() => handleFactory.from_bytes(bytes, config, expected)),
      secret,
      config[10],
      config[0],
    );
  }

  static async fromStream(
    body: ReadableStream<Uint8Array>,
    options?: LoadOptions,
  ): Promise<AsearchIndex> {
    requireBinding();
    const { config, maxBytes, expected } = loadConfig(options);
    try {
      return new AsearchIndex(
        await loadStream(body, config, maxBytes, expected),
        secret,
        config[10],
        config[0],
      );
    } catch (error) {
      return call(() => {
        throw error;
      });
    }
  }

  search(pattern: string, options?: SearchOptions): SearchResult {
    requireHandle(this);
    if (searching.has(this))
      fail("CONFLICT", "AsearchIndex search is already in progress");
    searching.add(this);
    try {
      const prepared = searchConfig(options);
      return duringSearch(this, () =>
        executeSearch(
          () => requireHandle(this),
          pattern,
          prepared,
          patternLimits.get(this) ?? 0,
          profiles.get(this) ?? "ideal",
        ),
      );
    } finally {
      searching.delete(this);
    }
  }

  session(
    options: SearchOptions & { maxCacheBytes?: number } = {},
  ): SearchSession {
    requireHandle(this);
    checkKeys(
      options,
      [
        "mode",
        "wrap",
        "maxAmbig",
        "limit",
        "countTotal",
        "engine",
        "maxCacheBytes",
      ],
      "session options",
    );
    const cacheOption = options.maxCacheBytes;
    const searchOptions: SearchOptions = {
      mode: options.mode,
      wrap: options.wrap,
      maxAmbig: options.maxAmbig,
      limit: options.limit,
      countTotal: options.countTotal,
      engine: options.engine,
    };
    const maxCacheBytes = Math.min(
      integer(optional(cacheOption, 32 * 1024 * 1024), "maxCacheBytes"),
      0xffffffff,
    );
    const prepared = searchConfig(searchOptions);
    let native: IndexHandle | null = call(() =>
      requireHandle(this).fork_session(
        searchOptions.engine === "transposed-blocked" ? 0 : maxCacheBytes,
        prepared.callback ? 7 : prepared.config[3],
      ),
    );
    const resources = children.get(this);
    if (!resources) throw new TypeError("Invalid AsearchIndex receiver");
    const resource = {
      clear() {
        native?.clear_workspace();
      },
      free() {
        const old = native;
        native = null;
        resources.delete(resource);
        old?.free();
      },
    };
    resources.add(resource);
    const getHandle = (): IndexHandle => {
      requireHandle(this);
      if (!native) fail("DISPOSED", "SearchSession has been disposed");
      return native;
    };
    return SearchSession.create({
      update: (pattern) =>
        duringSearch(this, () =>
          executeSearch(
            getHandle,
            pattern,
            prepared,
            patternLimits.get(this) ?? 0,
            profiles.get(this) ?? "ideal",
          ),
        ),
      reset: () => call(() => getHandle().reset_session()),
      free: () => resource.free(),
    });
  }

  explain(
    reference: number,
    pattern: string,
    options: SearchOptions & { original?: string } = {},
  ): Explanation | null {
    requireHandle(this);
    if (searching.has(this))
      fail("CONFLICT", "AsearchIndex search is already in progress");
    searching.add(this);
    try {
      return duringSearch(this, () => {
        return explainSearch(
          () => requireHandle(this),
          reference,
          pattern,
          options,
          patternLimits.get(this) ?? 0,
          profiles.get(this) ?? "ideal",
        );
      });
    } finally {
      searching.delete(this);
    }
  }

  segments(): SegmentInfo[] {
    const packed = call(() => requireHandle(this).segments_packed());
    const parts: SegmentInfo[] = [];
    for (let i = 0; i < packed.length; i += 5)
      parts.push({
        id:
          packed[i + 1].toString(16).padStart(8, "0") +
          packed[i].toString(16).padStart(8, "0"),
        bytes: packed[i + 2],
        texts: packed[i + 3],
        dead: packed[i + 4],
      });
    return parts;
  }

  async addSegment(
    source: Uint8Array | ReadableStream<Uint8Array>,
    expected?: ManifestSegment,
  ): Promise<SegmentInfo> {
    await this.updateSegments([{ source, expected }], true);
    const parts = this.segments();
    return parts[parts.length - 1];
  }

  async replaceSegments(sources: readonly SegmentSource[]): Promise<void> {
    await this.updateSegments(sources, false);
  }

  private async updateSegments(
    sources: readonly SegmentSource[],
    append: boolean,
  ): Promise<void> {
    requireHandle(this);
    if (updates.has(this) || (activeSearches.get(this) ?? 0) > 0)
      fail("BUSY", "Index update already in progress or search is executing");
    const controller = new AbortController();
    updates.set(this, controller);
    try {
      if (!Array.isArray(sources))
        throw new TypeError("sources must be an array");
      const count = integer(sources.length, "sources.length", 1, 8);
      const snapshot: UpdateSource[] = [];
      for (let i = 0; i < count; i++) {
        const entry = sources[i];
        checkKeys(entry, ["source", "expected"], "segment source");
        const { source, expected } = entry;
        if (
          !(source instanceof Uint8Array) &&
          !(source instanceof ReadableStream)
        )
          throw new TypeError("Expected Uint8Array or ReadableStream");
        snapshot.push({ source, expected: expectedWire(expected) });
      }
      await call(() =>
        applyUpdate(
          {
            signal: controller.signal,
            check: () => {
              requireHandle(this);
            },
            clear: () => {
              requireHandle(this).clear_workspace();
              for (const child of children.get(this) ?? []) child.clear();
            },
            begin: () => call(() => requireHandle(this).begin_update(append)),
          },
          snapshot,
        ),
      );
    } catch (error) {
      call(() => {
        throw error;
      });
    } finally {
      updates.delete(this);
    }
  }

  toBytes(): Uint8Array {
    const handle = requireHandle(this);
    return call(() => handle.to_bytes());
  }

  free(): void {
    const handle = handles.get(this);
    if (handle === undefined)
      throw new TypeError("Invalid AsearchIndex receiver");
    if (handle === null) return;
    handles.set(this, null);
    updates.get(this)?.abort();
    for (const child of children.get(this) ?? []) child.free();
    finalizer.unregister(this);
    handle.free();
  }

  declare [Symbol.dispose]: () => void;
}

Object.defineProperty(
  AsearchIndex.prototype,
  Symbol.dispose ?? Symbol.for("Symbol.dispose"),
  { value: AsearchIndex.prototype.free, configurable: true, writable: true },
);

export function buildIndex(
  docs: Iterable<Doc>,
  options?: IndexOptions,
): Uint8Array {
  const index = AsearchIndex.fromDocs(docs, options);
  try {
    return index.toBytes();
  } finally {
    index.free();
  }
}

/** Builds one immutable server segment. Does not modify any active index. */
export function buildDelta(
  docs: Iterable<Doc>,
  tombstones: readonly number[],
  options?: IndexOptions,
): Uint8Array {
  const factory = requireBinding();
  const config = indexConfig(options);
  if (!Array.isArray(tombstones))
    throw new TypeError("tombstones must be an array");
  const count = integer(tombstones.length, "tombstones.length");
  if (count > Math.floor(Math.min(config[5], config[9]) / 4))
    fail("RESOURCE_LIMIT", "Tombstones exceed input budget");
  const keys = new Uint32Array(count);
  for (let i = 0; i < count; i++)
    keys[i] = integer(tombstones[i], "tombstone key", 0, 0xffffffff);
  const flat = flatten(docs, config);
  return call(() =>
    factory.build_delta_flat(
      flat.units,
      flat.lengths,
      flat.keys,
      flat.orders,
      keys,
      config,
    ),
  );
}
