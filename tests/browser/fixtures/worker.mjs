import { Asearch } from "../../../packages/asearch-wasm/dist/web.mjs";

self.postMessage({
  matched: new Asearch("猫である", 1).match("吾輩は猫である。"),
  process: typeof process,
});
