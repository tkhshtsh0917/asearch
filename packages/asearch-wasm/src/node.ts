import bindings from "../dist/node/asearch_wasm.js";
import { installIndexConstructor } from "./bulk.js";

export { AsearchIndex, buildIndex, byLength, fixed } from "./bulk.js";

import { type InitInput, installConstructor } from "./core.js";

export type { InitInput, Options } from "./core.js";
export { Asearch, MatchMode } from "./core.js";

installConstructor(bindings.AsearchHandle);
installIndexConstructor(bindings.IndexHandle);
export const ready: Promise<void> = Promise.resolve();
export function init(_input?: InitInput): Promise<void> {
  return ready;
}
