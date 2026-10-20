import bindings from "../dist/node/asearch_wasm.js";
import { installIntentConstructor } from "./intent.js";

export { IntentIndex } from "./intent.js";

import { installIndexConstructor } from "./bulk.js";
import { installStreamBindings } from "./index-stream.js";

export {
  AsearchIndex,
  buildDelta,
  buildIndex,
  byLength,
  fixed,
  SearchSession,
} from "./bulk.js";

import { type InitInput, installConstructor } from "./core.js";

export type { InitInput, Options } from "./core.js";
export { Asearch, MatchMode } from "./core.js";

installConstructor(bindings.AsearchHandle);
installIndexConstructor(bindings.IndexHandle);
installIntentConstructor(bindings.IntentHandle);
installStreamBindings(bindings.StreamHandle, bindings.index_memory);
export const ready: Promise<void> = Promise.resolve();
export function init(_input?: InitInput): Promise<void> {
  return ready;
}

export { diffManifest } from "./manifest.js";

export { mergeResults } from "./merge-results.js";
