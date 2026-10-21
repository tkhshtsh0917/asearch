import type { Doc } from "./index-options.js";
import { fail, inputTypeError, integer, optional } from "./index-validation.js";

export function utf16(text: string): Uint16Array {
  if (typeof text !== "string")
    throw inputTypeError("Expected a primitive string");
  const units = new Uint16Array(text.length);
  for (let i = 0; i < text.length; i++) units[i] = text.charCodeAt(i);
  return units;
}

export function flatten(
  docs: Iterable<Doc>,
  config: Uint32Array,
  extraBytes = 0,
): {
  units: Uint16Array;
  lengths: Uint32Array;
  keys: Uint32Array;
  orders: Uint32Array;
} {
  if (config.byteLength + extraBytes > config[9])
    fail("RESOURCE_LIMIT", "Input transport exceeds working budget");
  const rows: { key: number; order: number; text: string }[] = [];
  const seen = new Set<number>();
  let total = 0;
  const iterator = docs?.[Symbol.iterator];
  if (typeof iterator !== "function")
    throw inputTypeError("docs must be iterable");
  const input = { [Symbol.iterator]: () => iterator.call(docs) };
  for (const doc of input) {
    if (typeof doc !== "object" || doc === null)
      throw inputTypeError("Expected a document object");
    const key = integer(doc.key, "key", 0, 0xffffffff);
    const order = integer(optional(doc.order, key), "order", 0, 0xffffffff);
    if (seen.has(key)) fail("INVALID_INPUT", "Duplicate document key");
    const texts = doc.texts;
    if (!Array.isArray(texts) || texts.length === 0)
      fail("INVALID_INPUT", "A document requires at least one text");
    const count = integer(texts.length, "texts length", 1, 0xffffffff);
    if (count > (config[6] ?? 0) - rows.length)
      fail("RESOURCE_LIMIT", "Document input exceeds text limit");
    seen.add(key);
    // Snapshot array elements, not an overrideable iterator that could
    // silently omit a document or yield more than its declared length.
    for (let i = 0; i < count; i++) {
      const text = texts[i];
      if (typeof text !== "string")
        throw inputTypeError("Expected a primitive text string");
      total += text.length;
      if (
        text.length > 65535 ||
        total > (config[5] ?? 0) / 4 ||
        total * 2 + (rows.length + 1) * 12 + config.byteLength + extraBytes >
          config[9] ||
        rows.length >= (config[6] ?? 0)
      )
        fail("RESOURCE_LIMIT", "Document input exceeds limits");
      rows.push({ key, order, text });
    }
  }
  const units = new Uint16Array(total);
  const lengths = new Uint32Array(rows.length);
  const keys = new Uint32Array(rows.length);
  const orders = new Uint32Array(rows.length);
  let start = 0;
  for (let i = 0; i < rows.length; i++) {
    const { key, order, text } = rows[i];
    lengths[i] = text.length;
    keys[i] = key;
    orders[i] = order;
    for (let j = 0; j < text.length; j++) units[start++] = text.charCodeAt(j);
  }
  return { units, lengths, keys, orders };
}
