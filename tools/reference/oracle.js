// Runs the REAL published packages (asearch.ts@1.0.3 lib, asearch@1.0.1) on JSON cases.
// usage: node oracle.js <node_modules dir> < cases.json > results.json
const path = require("node:path");
const nm = path.resolve(process.argv[2]); // relative paths are resolved against the cwd
const { Asearch } = require(path.join(nm, "asearch.ts"));
const NodeAsearch = require(path.join(nm, "asearch"));
let input = "";
process.stdin.on("data", (d) => (input += d));
process.stdin.on("end", () => {
  const cases = JSON.parse(input);
  const out = cases.map((c) => {
    let ts = null,
      tsErr = null;
    try {
      const a = new Asearch(c.p, c.mode);
      ts = c.texts.map((t) => c.ks.map((k) => a.match(t, k)));
    } catch (e) {
      tsErr = e.message;
    }
    const nmatch = NodeAsearch(c.p);
    const node = c.texts.map((t) => c.ks.map((k) => nmatch(t, k)));
    return { ts, tsErr, node };
  });
  process.stdout.write(JSON.stringify(out));
});
