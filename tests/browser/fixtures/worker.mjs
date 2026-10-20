import {
  Asearch,
  AsearchIndex,
} from "../../../packages/asearch-wasm/dist/web.mjs";

const matcher = new Asearch("猫である", 1);
const index = AsearchIndex.fromStrings(["abcdef", "がく", "😀abc"]);
const session = index.session({ countTotal: true });
try {
  session.update("a");
  session.update("ab");
  const found = session.update("abc");
  self.postMessage({
    matched: matcher.match("吾輩は猫である。"),
    process: typeof process,
    session: [...found.ids],
    optimized: found.stats.reusedRows > 0 || found.stats.engine === "narrowing",
  });
} finally {
  session.free();
  index.free();
  matcher.free();
}
