import { MatchMode } from "./pattern.js";

/** UTF-16 decoding must preserve lone surrogates; TextDecoder would replace
 * them. Small chunks also avoid function-argument limits for long patterns. */
export function casingKeys(units: Uint16Array, mode: MatchMode): Uint32Array {
  const symbols = Array.from(fromUtf16(units));
  const keys = new Uint32Array(symbols.length * 3);
  const single = (key: string): number =>
    Array.from(key).length === 1
      ? (key.codePointAt(0) ?? 0xffffffff)
      : 0xffffffff;
  for (let i = 0; i < symbols.length; i++) {
    const c = symbols[i];
    keys[i * 3] = single(c);
    if (mode === MatchMode.WildcardSpace && c === " ") {
      keys[i * 3 + 1] = 0xffffffff;
      keys[i * 3 + 2] = 0xffffffff;
      continue;
    }
    keys[i * 3 + 1] = single(c.toLocaleLowerCase());
    keys[i * 3 + 2] = single(c.toUpperCase());
  }
  return keys;
}

export function fromUtf16(units: Uint16Array): string {
  const chunks: string[] = [];
  for (let i = 0; i < units.length; i += 8192)
    chunks.push(String.fromCharCode(...units.subarray(i, i + 8192)));
  return chunks.join("");
}
