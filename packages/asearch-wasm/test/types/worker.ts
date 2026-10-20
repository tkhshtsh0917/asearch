import { Asearch, init, ready } from "../../dist/index.js";

// Compile against WebWorker without DOM: the public API must not require Window.
async function search() {
  await ready;
  await init();
  return new Asearch("hello").match("helo", 1);
}
void search;

import { IntentIndex } from "../../dist/index.js";

using intent = IntentIndex.fromEntries([{ key: 1, expr: "a|b" }]);
const found = intent.search("a", { engine: "dag" });
void intent.explain(found.refs[0], "a");
