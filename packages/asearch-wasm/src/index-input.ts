import { type Doc, fail, integer } from "./index-options.js";

export function utf16(text: string): Uint16Array {
  if (typeof text !== "string")
    throw new TypeError("Expected a primitive string");
  const units = new Uint16Array(text.length);
  for (let i = 0; i < text.length; i++) units[i] = text.charCodeAt(i);
  return units;
}

export function flatten(
  docs: Iterable<Doc>,
  config: Uint32Array,
): {
  units: Uint16Array;
  lengths: Uint32Array;
  keys: Uint32Array;
  orders: Uint32Array;
} {
  const rows: { key: number; order: number; text: string }[] = [];
  const seen = new Set<number>();
  let total = 0;
  for (const doc of docs) {
    const key = integer(doc.key, "key", 0, 0xffffffff);
    const order = integer(doc.order ?? key, "order", 0, 0xffffffff);
    if (seen.has(key)) fail("INVALID_INPUT", "Duplicate document key");
    seen.add(key);
    const texts = doc.texts;
    if (!Array.isArray(texts) || texts.length === 0)
      fail("INVALID_INPUT", "A document requires at least one text");
    for (const text of texts) {
      if (typeof text !== "string")
        throw new TypeError("Expected a primitive text string");
      total += text.length;
      if (
        text.length > 65535 ||
        total > (config[5] ?? 0) / 4 ||
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
