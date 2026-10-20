export interface SearchStats {
  engine: "transposed-blocked" | "incremental" | "narrowing" | "mixed" | "dag";
  segments: number;
  blocks: number;
  earlyExitBlocks: number;
  reusedRows: number;
  narrowedTexts?: number;
}
/** Current bulk results are JS-owned copies, not views over WASM memory. */
export interface SearchResult {
  ids: Uint32Array;
  levels: Uint8Array;
  refs: Uint32Array;
  orders: Uint32Array;
  length: number;
  totals?: Uint32Array;
  stats: SearchStats;
  copy(): SearchResult;
}

export function unpack(packed: Uint32Array): SearchResult {
  const length = packed[0];
  const totalsLength = packed[1];
  const ids = new Uint32Array(length);
  const orders = new Uint32Array(length);
  const refs = new Uint32Array(length);
  const levels = new Uint8Array(length);
  for (let i = 0; i < length; i++) {
    ids[i] = packed[8 + i * 4];
    orders[i] = packed[9 + i * 4];
    refs[i] = packed[10 + i * 4];
    levels[i] = packed[11 + i * 4];
  }
  const totals = totalsLength ? packed.slice(8 + length * 4) : undefined;
  return result(ids, levels, refs, orders, totals, {
    engine:
      packed[5] === 4
        ? "dag"
        : packed[5] === 3
          ? "mixed"
          : packed[5] === 2
            ? "narrowing"
            : packed[5] === 1
              ? "incremental"
              : "transposed-blocked",
    segments: packed[6],
    blocks: packed[2],
    earlyExitBlocks: packed[3],
    reusedRows: packed[4],
    ...(packed[5] === 2 || packed[7] > 0 ? { narrowedTexts: packed[7] } : {}),
  });
}

function result(
  ids: Uint32Array,
  levels: Uint8Array,
  refs: Uint32Array,
  orders: Uint32Array,
  totals: Uint32Array | undefined,
  stats: SearchStats,
): SearchResult {
  return {
    ids,
    levels,
    refs,
    orders,
    length: ids.length,
    ...(totals ? { totals } : {}),
    stats,
    copy() {
      return result(
        ids.slice(),
        levels.slice(),
        refs.slice(),
        orders.slice(),
        totals?.slice(),
        { ...stats },
      );
    },
  };
}
