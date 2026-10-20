import { Asearch, init, ready } from "../../dist/index.js";

// Compile against WebWorker without DOM: the public API must not require Window.
async function search() {
  await ready;
  await init();
  return new Asearch("hello").match("helo", 1);
}
void search;
