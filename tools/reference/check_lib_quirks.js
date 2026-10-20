// Run with: node check_lib_quirks.js <path-to-asearch.ts-lib-index.js>
const fs = require("node:fs");
const vm = require("node:vm");
const libPath = require("node:path").resolve(process.argv[2]); // relative paths are resolved against the cwd
const { Asearch } = require(libPath);

// 1) Emoji in candidate text (README claims emoji support)
const a = new Asearch("a😀b");
console.log(
  "emoji exact a😀b vs a😀b ambig0:",
  a.match("a😀b", 0),
  " ambig1:",
  a.match("a😀b", 1),
  " ambig2:",
  a.match("a😀b", 2),
);
const e = new Asearch("😀");
console.log("pattern 😀 vs 😀 ambig0:", e.match("😀", 0));

// 2) Length check counts UTF-16 units
try {
  new Asearch("😀".repeat(16));
  console.log("16 emoji (32 units, 16 code points): accepted");
} catch (err) {
  console.log(
    "16 emoji: threw ->",
    Array.from(err.message).slice(0, 60).join(""),
  );
}

// 3) Non-monotonic ambiguity on empty / short candidates
const p = new Asearch("a");
console.log(
  'pattern a vs "" : ambig0',
  p.match("", 0),
  "ambig1",
  p.match("", 1),
  "ambig2",
  p.match("", 2),
  "ambig3",
  p.match("", 3),
);
const q = new Asearch("ab");
console.log(
  'pattern ab vs "" :',
  [0, 1, 2, 3, 4].map((k) => q.match("", k)).join(","),
);

// 4) Browser-like context without `process`
const code = fs.readFileSync(libPath, "utf8");
const sandbox = { exports: {}, console };
sandbox.module = { exports: sandbox.exports };
vm.createContext(sandbox);
vm.runInContext(code, sandbox);
try {
  const B = new sandbox.exports.Asearch("abc");
  console.log("no-process context match:", B.match("abc"));
} catch (err) {
  console.log(
    "no-process context match: threw ->",
    `${err.constructor.name}: ${err.message}`,
  );
}
// 5) ambig >= 32 shift wrap
const w = new Asearch("abc");
console.log(
  'ambig 31/32/33 on "abc":',
  w.match("abc", 31),
  w.match("abc", 32),
  w.match("abc", 33),
);
console.log("ambig 1.5 / -1:", w.match("abc", 1.5), w.match("abc", -1));
