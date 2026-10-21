import { typedArrayKind, typedArrayLength } from "./index-bytes.js";
import type { SearchResult } from "./index-result.js";
import { fail, integer } from "./index-validation.js";

/** Structured-clone friendly subset; Worker messages cannot carry copy(). */
export type MergePart = Pick<
  SearchResult,
  "ids" | "levels" | "refs" | "orders" | "length"
>;
export interface MergedResults {
  ids: Uint32Array;
  levels: Uint8Array;
  orders: Uint32Array;
  parts: Uint8Array;
  refs: Uint32Array;
}
function before(a: MergePart, i: number, b: MergePart, j: number): boolean {
  return (
    a.levels[i] < b.levels[j] ||
    (a.levels[i] === b.levels[j] &&
      (a.orders[i] < b.orders[j] ||
        (a.orders[i] === b.orders[j] && a.ids[i] < b.ids[j])))
  );
}
function snapshot(value: MergePart): MergePart {
  if (typeof value !== "object" || value === null)
    fail("INVALID_INPUT", "Expected result part");
  const { ids, levels, orders, refs, length } = value;
  return { ids, levels, orders, refs, length };
}

function validate(value: MergePart): void {
  const { ids, levels, orders, refs, length } = value;
  if (
    typedArrayKind(ids) !== "Uint32Array" ||
    typedArrayKind(orders) !== "Uint32Array" ||
    typedArrayKind(refs) !== "Uint32Array" ||
    typedArrayKind(levels) !== "Uint8Array"
  )
    fail("INVALID_INPUT", "Result parts require typed arrays");
  integer(length, "part length", 0, 0xffffffff);
  if (
    [ids, levels, orders, refs].map(typedArrayLength).some((n) => n !== length)
  )
    fail("INVALID_INPUT", "Result array lengths differ");
  const result = { ids, levels, orders, refs, length };
  for (let i = 0; i < length; i++) {
    if (levels[i] > 7 || (i > 0 && before(result, i, result, i - 1)))
      fail("INVALID_INPUT", "Result part is not in minimum-level rank order");
  }
}

/** Static Worker partitions only. Never sum shard totals or reuse returned
 * refs in another shard: parts[i] selects the owner of refs[i]. */
export function mergeResults(
  parts: readonly MergePart[],
  limit: number,
): MergedResults {
  if (!Array.isArray(parts))
    fail("INVALID_INPUT", "Expected at most 256 result parts");
  integer(limit, "limit");
  const inputs: MergePart[] = [];
  let total = 0;
  const count = integer(parts.length, "part count", 0, 256);
  for (let i = 0; i < count; i++) {
    const part = snapshot(parts[i]);
    inputs.push(part);
  }
  // All caller getters have completed before validating any retained arrays.
  for (const part of inputs) {
    validate(part);
    total += part.length;
  }
  const capacity = Math.min(limit, total);
  // This bounds output typed-array bytes, not the host's entire JS heap.
  if (capacity > Math.floor((256 * 1024 * 1024) / 14))
    fail("RESOURCE_LIMIT", "Merged output exceeds 256 MiB");
  const ids = new Uint32Array(capacity),
    levels = new Uint8Array(capacity),
    orders = new Uint32Array(capacity),
    owners = new Uint8Array(capacity),
    refs = new Uint32Array(capacity);
  const cursors = new Uint32Array(count);
  const seen = new Set<number>();
  let length = 0;
  while (length < capacity) {
    let selected = -1;
    for (let p = 0; p < count; p++) {
      const part = inputs[p];
      while (cursors[p] < part.length && seen.has(part.ids[cursors[p]]))
        cursors[p]++;
      if (cursors[p] === part.length) continue;
      if (
        selected < 0 ||
        before(part, cursors[p], inputs[selected], cursors[selected])
      )
        selected = p;
    }
    if (selected < 0) break;
    const part = inputs[selected],
      position = cursors[selected]++;
    ids[length] = part.ids[position];
    levels[length] = part.levels[position];
    orders[length] = part.orders[position];
    owners[length] = selected;
    refs[length] = part.refs[position];
    seen.add(ids[length]);
    length++;
  }
  return {
    ids: ids.subarray(0, length),
    levels: levels.subarray(0, length),
    orders: orders.subarray(0, length),
    parts: owners.subarray(0, length),
    refs: refs.subarray(0, length),
  };
}
