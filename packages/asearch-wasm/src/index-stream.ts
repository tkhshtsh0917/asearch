import type { IndexHandle } from "./index-bindings.js";
import { byteView } from "./index-bytes.js";
import { fail } from "./index-validation.js";
import { readerFor } from "./stream-reader.js";

interface StreamHandle {
  input_ptr(): number;
  finish(expected: Uint8Array): IndexHandle;
  free(): void;
}
export interface StreamConstructor {
  new (header: Uint8Array, config: Uint32Array): StreamHandle;
}

let streamConstructor: StreamConstructor | undefined;
let memory: (() => WebAssembly.Memory) | undefined;
export function installStreamBindings(
  factory: StreamConstructor,
  getMemory: () => WebAssembly.Memory,
): void {
  streamConstructor = factory;
  memory = getMemory;
}

export function indexMemory(): WebAssembly.Memory {
  if (!memory)
    throw new Error("asearch-wasm is not initialized; await init() first");
  return memory();
}

export async function loadStream(
  body: ReadableStream<Uint8Array>,
  config: Uint32Array,
  maxBytes: number,
  expected: Uint8Array,
): Promise<IndexHandle> {
  if (!streamConstructor || !memory)
    throw new Error("asearch-wasm is not initialized; await init() first");
  const Constructor = streamConstructor;
  const getMemory = memory;
  const reader = readerFor(body);
  const header = new Uint8Array(64);
  let headerBytes = 0;
  let declared = 0;
  let written = 0;
  let handle: StreamHandle | undefined;
  let consumed = false;
  let complete = false;
  try {
    for (;;) {
      const { done, value: chunk } = await reader.read();
      if (done) break;
      const value = byteView(chunk);
      let offset = 0;
      if (!handle) {
        offset = Math.min(64 - headerBytes, value.length);
        header.set(value.subarray(0, offset), headerBytes);
        headerBytes += offset;
        if (headerBytes < 64) continue;
        const view = new DataView(header.buffer);
        if (
          view.getUint32(0, true) !== 0x58495341 ||
          view.getUint16(4, true) !== 1
        )
          fail("INVALID_INDEX", "Invalid ASIX streaming header");
        const length = view.getBigUint64(16, true);
        if (
          length < 64n ||
          length > BigInt(Math.min(maxBytes, config[5], 0xffffffff))
        )
          fail("RESOURCE_LIMIT", "Stream exceeds input byte limit");
        declared = Number(length);
        handle = new Constructor(header, config);
        written = 64;
      }
      const remaining = value.length - offset;
      if (remaining > declared - written)
        fail("INVALID_INDEX", "Stream contains extra bytes");
      // An await may allow another index to grow WASM memory. Never retain a
      // TypedArray view across that boundary, even when the pointer is stable.
      new Uint8Array(getMemory().buffer, handle.input_ptr(), declared).set(
        value.subarray(offset),
        written,
      );
      written += remaining;
    }
    if (!handle || written !== declared)
      fail("INVALID_INDEX", "Truncated ASIX stream");
    consumed = true; // finish consumes the native handle even when validation throws.
    const index = handle.finish(expected);
    complete = true;
    return index;
  } finally {
    // A caller-provided cancel hook can remain pending forever. Native memory
    // and the reader lock must be released without awaiting that hook.
    if (!complete) void reader.cancel().catch(() => {});
    reader.releaseLock();
    if (handle && !consumed) handle.free();
  }
}
