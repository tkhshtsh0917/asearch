// Mechanical "source semantics" oracle for the AsearchTs profile (D1): transpile the vendored
// asearch.ts src/index.ts at ES2020 (for..of iterates code points) and run it next to the published lib.
// usage: node tools/reference/src_oracle.cjs vendor/upstream
const fs = require("node:fs"),
  path = require("node:path"),
  vm = require("node:vm"),
  ts = require(
    require.resolve("typescript", {
      paths: [path.join(__dirname, "../fixtures")],
    }),
  );
const src = fs.readFileSync(
  path.resolve(process.argv[2], "asearch.ts/src/index.ts"),
  "utf8",
);
const out = ts.transpileModule(src, {
  compilerOptions: {
    target: ts.ScriptTarget.ES2020,
    module: ts.ModuleKind.CommonJS,
  },
}).outputText;
const mod = { exports: {} };
vm.runInNewContext(out, {
  module: mod,
  exports: mod.exports,
  process: { env: {} },
  console,
});
const { Asearch } = mod.exports;
const lib = require(path.resolve(process.argv[2], "asearch.ts"));
console.log("typescript", ts.version);
console.log(
  "src  😀 vs 😀",
  new Asearch("😀").match("😀"),
  " a😀b vs a😀b",
  new Asearch("a😀b").match("a😀b"),
);
console.log(
  "lib  😀 vs 😀",
  new lib.Asearch("😀").match("😀"),
  " a😀b vs a😀b",
  new lib.Asearch("a😀b").match("a😀b"),
);
console.log(
  "src  abc vs xabcx Include,1",
  new Asearch("abc", 1).match("xabcx", 1),
  "lib",
  new lib.Asearch("abc", 1).match("xabcx", 1),
);
