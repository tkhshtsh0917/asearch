import { byteView } from "./index-bytes.js";
import { indexMemory } from "./index-stream.js";
import { fail } from "./index-validation.js";
import { readerFor } from "./stream-reader.js";

export interface UpdateHandle {
  allocate(header: Uint8Array, expected: Uint8Array): number;
  complete_source(): void;
  finish(): void;
  free(): void;
}
export interface UpdateSource {
  source: Uint8Array | ReadableStream<Uint8Array>;
  expected: Uint8Array;
}
export interface UpdateBackend {
  begin(): UpdateHandle;
  check(): void;
  transport(bytes: number): void;
  clear(): void;
  signal: AbortSignal;
}

async function receive(
  entry: UpdateSource,
  handle: UpdateHandle,
  backend: UpdateBackend,
): Promise<void> {
  const body =
    entry.source instanceof Uint8Array
      ? new ReadableStream<Uint8Array>({
          start(controller) {
            controller.enqueue(entry.source as Uint8Array);
            controller.close();
          },
        })
      : entry.source;
  const reader = readerFor(body);
  const header = new Uint8Array(64);
  let headerBytes = 0,
    declared = 0,
    written = 0,
    pointer: number | undefined;
  let complete = false;
  let rejectAbort: (reason: unknown) => void = () => {};
  const aborted = new Promise<never>((_, reject) => {
    rejectAbort = reject;
  });
  const onAbort = () => {
    rejectAbort(
      Object.assign(new Error("AsearchIndex has been disposed"), {
        code: "DISPOSED",
      }),
    );
    void reader.cancel().catch(() => {});
  };
  backend.signal.addEventListener("abort", onAbort, { once: true });
  try {
    backend.check();
    if (backend.signal.aborted) onAbort();
    for (;;) {
      const { done, value: chunk } = await Promise.race([
        reader.read(),
        aborted,
      ]);
      backend.check();
      if (done) break;
      const value = byteView(chunk);
      let offset = 0;
      if (pointer === undefined) {
        offset = Math.min(64 - headerBytes, value.length);
        header.set(value.subarray(0, offset), headerBytes);
        headerBytes += offset;
        if (headerBytes < 64) continue;
        // Native validation checks header counts and reserves the complete
        // transport buffer against old data and every live session.
        backend.transport(header.byteLength + entry.expected.byteLength);
        pointer = handle.allocate(header, entry.expected);
        declared = Number(new DataView(header.buffer).getBigUint64(16, true));
        written = 64;
      }
      if (value.length - offset > declared - written)
        fail("INVALID_INDEX", "Stream contains extra bytes");
      new Uint8Array(indexMemory().buffer, pointer, declared).set(
        value.subarray(offset),
        written,
      );
      written += value.length - offset;
    }
    if (pointer === undefined || written !== declared)
      fail("INVALID_INDEX", "Truncated ASIX update stream");
    handle.complete_source();
    complete = true;
  } finally {
    backend.signal.removeEventListener("abort", onAbort);
    if (!complete) void reader.cancel().catch(() => {});
    reader.releaseLock();
  }
}

export async function applyUpdate(
  backend: UpdateBackend,
  sources: readonly UpdateSource[],
): Promise<void> {
  backend.check();
  backend.clear();
  const handle = backend.begin();
  let consumed = false;
  try {
    for (const source of sources) await receive(source, handle, backend);
    backend.check();
    // Searches during awaits may have prepared new workspaces. Release those
    // too; all query inputs and search options remain owned by their handles.
    backend.clear();
    consumed = true;
    handle.finish();
  } finally {
    if (!consumed) handle.free();
  }
}
