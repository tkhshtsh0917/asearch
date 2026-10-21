import { inputTypeError } from "./index-validation.js";
/** Use the branded stream operation, not an input object's overridable reader
 * factory. Underlying pull/cancel callbacks still follow native stream rules. */
export function readerFor(
  body: ReadableStream<Uint8Array>,
): ReadableStreamDefaultReader<Uint8Array> {
  try {
    return ReadableStream.prototype.getReader.call(
      body,
    ) as ReadableStreamDefaultReader<Uint8Array>;
  } catch {
    throw inputTypeError("Expected an unlocked ReadableStream");
  }
}
