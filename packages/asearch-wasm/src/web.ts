import initialize, { AsearchHandle } from "../dist/web/asearch_wasm.js";
import { type InitInput, installConstructor } from "./core.js";

export type { InitInput, Options } from "./core.js";
export { Asearch, MatchMode } from "./core.js";

let state: "uninitialized" | "initializing" | "ready" | "failed" =
  "uninitialized";
export let ready: Promise<void>;
function failure(code: string, message: string, cause?: unknown): Error {
  return Object.assign(new Error(message, { cause }), { code });
}
export function init(input?: InitInput): Promise<void> {
  if (state === "initializing" || state === "ready") {
    if (input !== undefined) {
      return Promise.reject(
        failure(
          state === "ready"
            ? "ALREADY_INITIALIZED"
            : "INITIALIZATION_IN_PROGRESS",
          "Initialization already started",
        ),
      );
    }
    return ready;
  }
  state = "initializing";
  ready = Promise.resolve().then(async () => {
    try {
      await initialize(
        input === undefined ? undefined : { module_or_path: input },
      );
      installConstructor(AsearchHandle);
      state = "ready";
    } catch (cause) {
      state = "failed";
      throw failure(
        "INITIALIZATION_FAILED",
        "Could not initialize asearch-wasm",
        cause,
      );
    }
  });
  void ready.catch(() => {});
  return ready;
}
await init().catch(() => {});
