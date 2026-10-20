export enum MatchMode {
  Exact = 0,
  Include = 1,
  WildcardSpace = 2,
}

export interface CompiledPattern {
  loopMask: number;
  masks: Record<string, number>;
  matchMode: MatchMode;
  pattern: string;
  initPattern: number;
  acceptPattern: number;
}

// Keep property insertion order and JS casing identical to the upstream compiler.
export function compilePattern(
  pattern: string,
  mode: MatchMode,
  maximumLength: number,
): CompiledPattern {
  const self = { loopMask: 0, masks: {} } as CompiledPattern;
  if (pattern.length > maximumLength) {
    throw new Error(
      `Pattern must be shorter than ${maximumLength + 1} chars. But given ${pattern}`,
    );
  }
  self.matchMode = mode;
  self.pattern = pattern;

  const splitted = Array.from(pattern);
  const leftBit = 1 << maximumLength;
  self.initPattern = leftBit >>> 0;
  self.acceptPattern = leftBit >>> splitted.length;
  if (mode === MatchMode.Include)
    self.loopMask = self.initPattern | self.acceptPattern;

  let position = 0;
  for (const c of splitted) {
    if (mode === MatchMode.WildcardSpace && c === " ") {
      self.loopMask |= leftBit >>> position;
      self.acceptPattern <<= 1;
      continue;
    }
    for (const key of [c, c.toLocaleLowerCase(), c.toUpperCase()]) {
      self.masks[key] ??= 0;
      self.masks[key] |= leftBit >>> position;
    }
    position++;
  }

  return self;
}
