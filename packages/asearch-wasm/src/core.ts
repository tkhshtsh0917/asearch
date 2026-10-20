import { compilePattern, MatchMode } from "./pattern.js";

export { MatchMode } from "./pattern.js";

export type InitInput = WebAssembly.Module | BufferSource | URL;
export interface Options {
  profile?: "asearch.ts" | "asearch.ts-lib" | "ideal";
}
export interface Handle {
  match_u16(text: Uint16Array, ambig: number): boolean;
  free(): void;
  match_level_u16(text: Uint16Array, maxAmbig: number): number;
}
export interface HandleConstructor {
  new (
    keys: Uint32Array,
    masks: Uint32Array,
    loop: number,
    accept: number,
    initial: number,
    profile: number,
  ): Handle;
}

let handleConstructor: HandleConstructor | undefined;
const handles = new WeakMap<Asearch, Handle | null>();
export function installConstructor(value: HandleConstructor): void {
  handleConstructor = value;
}

function checkString(value: unknown): asserts value is string {
  if (typeof value === "string") return;

  try {
    // Check the actual String internal slot, including cross-realm objects.
    // Object.prototype.toString can be spoofed with Symbol.toStringTag.
    String.prototype.valueOf.call(value);
  } catch {
    throw new TypeError("Expected a string or String object");
  }
}

function utf16(query: string): Uint16Array {
  const text = String(query);
  const units = new Uint16Array(text.length);
  for (let i = 0; i < text.length; i++) units[i] = text.charCodeAt(i);
  return units;
}

function requireHandle(receiver: Asearch): Handle {
  const handle = handles.get(receiver);
  if (handle === null)
    throw Object.assign(new Error("Asearch has been disposed"), {
      code: "DISPOSED",
    });
  if (!handle) throw new TypeError("Invalid Asearch receiver");
  return handle;
}

export class Asearch {
  static MAXLENGTH = 31;
  declare readonly matchMode: MatchMode;
  declare readonly pattern: string;
  declare masks: Record<string, number>;

  constructor(
    pattern: string,
    mode: MatchMode = MatchMode.Exact,
    options?: Options,
  ) {
    if (!handleConstructor)
      throw new Error("asearch-wasm is not initialized; await init() first");
    checkString(pattern);

    const profile = options?.profile;
    if (
      profile !== undefined &&
      profile !== "asearch.ts" &&
      profile !== "asearch.ts-lib" &&
      profile !== "ideal"
    ) {
      throw new TypeError("Unknown asearch profile");
    }

    const compiled = compilePattern(pattern, mode, Asearch.MAXLENGTH);
    Object.assign(this, compiled);

    const entries = Object.entries(compiled.masks).filter(
      ([key]) => Array.from(key).length === 1,
    );

    handles.set(
      this,
      new handleConstructor(
        Uint32Array.from(entries, ([key]) => key.codePointAt(0) ?? 0),
        Uint32Array.from(entries, ([, mask]) => mask >>> 0),
        compiled.loopMask >>> 0,
        compiled.acceptPattern >>> 0,
        compiled.initPattern >>> 0,
        profile === "asearch.ts-lib" ? 1 : profile === "ideal" ? 2 : 0,
      ),
    );
  }

  matchLevel(query: string, maxAmbig = 3): number {
    requireHandle(this);
    checkString(query);
    if (typeof maxAmbig !== "number")
      throw new TypeError("maxAmbig must be a number");
    if (!Number.isInteger(maxAmbig) || maxAmbig < 0 || maxAmbig > 31)
      throw new RangeError("maxAmbig must be an integer in 0..31");
    const units = utf16(query);
    return requireHandle(this).match_level_u16(units, maxAmbig);
  }

  free(): void {
    const handle = handles.get(this);
    if (handle === null) return;
    if (!handle) throw new TypeError("Invalid Asearch receiver");
    handle.free();
    handles.set(this, null);
  }

  declare [Symbol.dispose]: () => void;

  match(query: string, ambig = 0): boolean {
    requireHandle(this);
    // Unary plus implements ToNumber (including TypeError for BigInt/Symbol).
    const rawLength = +(ambig + 1);
    const length =
      Number.isNaN(rawLength) || rawLength <= 0
        ? 0
        : Math.min(Math.floor(rawLength), Number.MAX_SAFE_INTEGER);
    if (length > 2 ** 32 - 1) throw new RangeError("Invalid array length");
    checkString(query);

    // Computed property names use ToPropertyKey, preserving a Symbol returned
    // by an object's @@toPrimitive. String(object) would incorrectly throw.
    const key =
      (typeof ambig === "object" && ambig !== null) ||
      typeof ambig === "function"
        ? Reflect.ownKeys({ [ambig]: 0 })[0]
        : String(ambig);
    requireHandle(this);
    if (typeof key !== "string") return false;
    const index = /^(0|[1-9][0-9]*)$/.test(key) ? Number(key) : -1;
    if (index < 0 || index >= length) return false;
    if (index > 65535)
      throw new RangeError("asearch-wasm: ambig must be <= 65535");

    const units = utf16(query);
    return requireHandle(this).match_u16(units, index);
  }
}

// Keep explicit free() usable even on hosts without native Symbol.dispose.
Object.defineProperty(
  Asearch.prototype,
  Symbol.dispose ?? Symbol.for("Symbol.dispose"),
  {
    value: Asearch.prototype.free,
    configurable: true,
    writable: true,
  },
);
