// Straight JS port of the vendored asearch.ts recurrence, without verbose/
// process checks. Benchmark baseline only; not an additional product engine.
// Upstream attribution and license are retained in the root NOTICE.
export class JsReplica {
  constructor(pattern, mode = 0) {
    if (pattern.length > 31)
      throw new RangeError("Pattern exceeds 31 UTF-16 units");
    this.initial = 1 << 31;
    this.accept = this.initial >>> Array.from(pattern).length;
    this.loops = mode === 1 ? this.initial | this.accept : 0;
    this.masks = {};
    let position = 0;
    for (const symbol of pattern) {
      if (mode === 2 && symbol === " ") {
        this.loops |= this.initial >>> position;
        this.accept <<= 1;
        continue;
      }
      for (const key of [
        symbol,
        symbol.toLocaleLowerCase(),
        symbol.toUpperCase(),
      ]) {
        this.masks[key] ??= 0;
        this.masks[key] |= this.initial >>> position;
      }
      position++;
    }
  }

  match(text, level = 0) {
    const states = Array.from(
      { length: level + 1 },
      (_, i) => this.initial >>> i,
    );
    for (let i = 0; i < text.length; i++) {
      const mask = this.masks[text[i]] ?? 0;
      for (let e = states.length - 1; e > 0; e--) {
        states[e] =
          (states[e] & this.loops) |
          ((states[e] & mask) >>> 1) |
          (states[e - 1] >>> 1) |
          states[e - 1];
      }
      states[0] = ((states[0] & mask) >>> 1) | (states[0] & this.loops);
      for (let e = 1; e < states.length; e++) states[e] |= states[e - 1] >>> 1;
    }
    return Boolean(states[level] & this.accept);
  }
}
