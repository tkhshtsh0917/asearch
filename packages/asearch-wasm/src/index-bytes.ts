import { inputTypeError } from "./index-validation.js";

const typedArrayPrototype = Object.getPrototypeOf(Uint8Array.prototype);
function accessor(name: string) {
  const getter = Object.getOwnPropertyDescriptor(
    typedArrayPrototype,
    name,
  )?.get;
  if (!getter) throw inputTypeError(`Missing TypedArray ${name} accessor`);
  return getter;
}
const getBuffer = accessor("buffer");
const getOffset = accessor("byteOffset");
const getLength = accessor("length");

/** Read internal slots, never caller-owned getters, methods or species. The
 * explicit length also prevents a growable backing buffer from enlarging the
 * view after a byte-budget check. Detached buffers and proxies fail safely. */
export function byteView(value: Uint8Array): Uint8Array {
  if (typedArrayKind(value) !== "Uint8Array")
    throw inputTypeError("Expected Uint8Array");

  try {
    return new Uint8Array(
      getBuffer.call(value),
      getOffset.call(value),
      getLength.call(value),
    );
  } catch {
    throw inputTypeError("Invalid or detached byte input");
  }
}

/** Use the actual element count when validating caller-owned result arrays. */
export function typedArrayLength(value: Uint8Array | Uint32Array): number {
  return getLength.call(value);
}

const getKind = accessorForTag();
function accessorForTag() {
  const getter = Object.getOwnPropertyDescriptor(
    typedArrayPrototype,
    Symbol.toStringTag,
  )?.get;
  if (!getter) throw inputTypeError("Missing TypedArray type accessor");
  return getter;
}
export function typedArrayKind(value: unknown): string | undefined {
  return getKind.call(value);
}
