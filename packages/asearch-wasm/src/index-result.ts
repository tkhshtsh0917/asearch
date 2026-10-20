export interface SearchStats {
  engine: "transposed-blocked" | "incremental";
  segments: number;
  blocks: number;
  earlyExitBlocks: number;
  reusedRows: number;
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
    ids[i] = packed[6 + i * 4];
    orders[i] = packed[7 + i * 4];
    refs[i] = packed[8 + i * 4];
    levels[i] = packed[9 + i * 4];
  }
  const totals = totalsLength ? packed.slice(6 + length * 4) : undefined;
  return result(ids, levels, refs, orders, totals, {
    engine: packed[5] ? "incremental" : "transposed-blocked",
    segments: 1,
    blocks: packed[2],
    earlyExitBlocks: packed[3],
    reusedRows: packed[4],
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
