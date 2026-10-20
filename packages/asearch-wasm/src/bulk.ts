import { indexConfig, loadConfig, searchConfig } from "./index-config.js";
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
import { checkKeys, fail, integer, optional } from "./index-validation.js";

export type {
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
  to_bytes(): Uint8Array;
  fork_session(maxCacheBytes: number): IndexHandle;
  reset_session(): void;
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
  from_bytes(bytes: Uint8Array, config: Uint32Array): IndexHandle;
}

let binding: IndexConstructor | undefined;
const secret = Symbol("AsearchIndex factory");
const handles = new WeakMap<AsearchIndex, IndexHandle | null>();
const searching = new WeakSet<AsearchIndex>();
const children = new WeakMap<AsearchIndex, Set<{ free(): void }>>();
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
  ) {
    if (token !== secret) throw new TypeError("Use an AsearchIndex factory");
    handles.set(this, handle);
    children.set(this, new Set());
    patternLimits.set(this, maxPatternUnits);
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
    const { config, maxBytes } = loadConfig(options);
    if (bytes.length > Math.min(config[5], maxBytes))
      fail("RESOURCE_LIMIT", "Index input exceeds byte limit");
    return new AsearchIndex(
      call(() => handleFactory.from_bytes(bytes, config)),
      secret,
      config[10],
    );
  }

  static async fromStream(
    body: ReadableStream<Uint8Array>,
    options?: LoadOptions,
  ): Promise<AsearchIndex> {
    requireBinding();
    const { config, maxBytes } = loadConfig(options);
    try {
      return new AsearchIndex(
        await loadStream(body, config, maxBytes),
        secret,
        config[10],
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
      return executeSearch(
        () => requireHandle(this),
        pattern,
        prepared,
        patternLimits.get(this) ?? 0,
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
      ),
    );
    const resources = children.get(this);
    if (!resources) throw new TypeError("Invalid AsearchIndex receiver");
    const resource = {
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
        executeSearch(
          getHandle,
          pattern,
          prepared,
          patternLimits.get(this) ?? 0,
        ),
      reset: () => call(() => getHandle().reset_session()),
      free: () => resource.free(),
    });
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
