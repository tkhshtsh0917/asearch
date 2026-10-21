const typedArrayPrototype = Object.getPrototypeOf(Uint8Array.prototype);
function accessor(name: string) {
  const getter = Object.getOwnPropertyDescriptor(
    typedArrayPrototype,
    name,
  )?.get;
  if (!getter) throw new TypeError(`Missing TypedArray ${name} accessor`);
  return getter;
}
const getBuffer = accessor("buffer");
const getOffset = accessor("byteOffset");
const getLength = accessor("length");

/** Read internal slots, never caller-owned getters, methods or species. The
 * explicit length also prevents a growable backing buffer from enlarging the
 * view after a byte-budget check. Detached buffers and proxies fail safely. */
export function byteView(value: Uint8Array): Uint8Array {
  if (!(value instanceof Uint8Array))
    throw new TypeError("Expected Uint8Array");

  return new Uint8Array(
    getBuffer.call(value),
    getOffset.call(value),
    getLength.call(value),
  );
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
  if (!getter) throw new TypeError("Missing TypedArray type accessor");
  return getter;
}
export function typedArrayKind(value: unknown): string | undefined {
  return getKind.call(value);
}
