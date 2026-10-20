import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";
import ts from "typescript";

const root = fileURLToPath(new URL("../../", import.meta.url));
const upstream = resolve(root, "vendor/upstream");
const hashes = {};
for (const line of (await readFile(resolve(upstream, "SHA256SUMS"), "utf8"))
  .trim()
  .split("\n")) {
  const [, expected, name] = /^(\S+)\s+(.+)$/.exec(line);
  const actual = createHash("sha256")
    .update(await readFile(resolve(upstream, name)))
    .digest("hex");
  if (actual !== expected) throw new Error(`Upstream changed: ${name}`);
  hashes[name] = actual;
}
const locale = Intl.DateTimeFormat().resolvedOptions().locale;
if (locale !== "en-US")
  throw new Error(`Fixtures require en-US; got ${locale}`);
function execute(code) {
  const module = { exports: {} };
  vm.runInNewContext(code, {
    module,
    exports: module.exports,
    process: { env: {} },
    console,
  });
  return module.exports;
}
const lib = execute(
  await readFile(resolve(upstream, "asearch.ts/lib/index.js"), "utf8"),
).Asearch;
const src = execute(
  ts.transpileModule(
    await readFile(resolve(upstream, "asearch.ts/src/index.ts"), "utf8"),
    {
      compilerOptions: {
        target: ts.ScriptTarget.ES2020,
        module: ts.ModuleKind.CommonJS,
      },
    },
  ).outputText,
).Asearch;
const node = execute(
  await readFile(resolve(upstream, "asearch/index.js"), "utf8"),
);
const units = (text) =>
  Array.from({ length: text.length }, (_, i) => text.charCodeAt(i));
const ambigs = [0, 1, 2, 3, 4, 5, 31, 32, 33];
const nodeAmbigs = [0, 1, 2, 3];
const cases = [];
function add(pattern, modeValue, texts) {
  const mode =
    modeValue === undefined
      ? { undefined: true }
      : Number.isNaN(modeValue)
        ? { nan: true }
        : { json: modeValue };
  const record = { id: `case-${cases.length}`, pattern: units(pattern), mode };
  let a, b, c;
  try {
    a = new lib(pattern, modeValue);
    b = new src(pattern, modeValue);
    c = node(pattern);
  } catch (error) {
    cases.push({
      ...record,
      ctor: { throws: { name: error.name, message: error.message } },
      texts: [],
      lib: [],
      src: [],
      node: [],
    });
    return;
  }
  record.ctor = {
    ok: {
      masks: Object.keys(a.masks).map((key) => [units(key), a.masks[key]]),
      loopMask: a.loopMask,
      initPattern: a.initPattern,
      acceptPattern: a.acceptPattern,
    },
  };
  record.texts = texts.map(units);
  for (const [key, matcher, levels] of [
    ["lib", a, ambigs],
    ["src", b, ambigs],
    ["node", { match: c }, nodeAmbigs],
  ]) {
    record[key] = texts.map((text) =>
      levels.map((level) => matcher.match(text, level)),
    );
  }
  cases.push(record);
}
const alphabet = [
  "a",
  "b",
  "A",
  " ",
  "漢",
  "😀",
  "ß",
  "Σ",
  "İ",
  "\ud83d",
  "\udc00",
];
let seed = 31;
function random(max) {
  seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
  return seed % max;
}
function string(max, chars = alphabet) {
  return Array.from(
    { length: random(max + 1) },
    () => chars[random(chars.length)],
  ).join("");
}
for (const pattern of [
  "",
  " ",
  "  ",
  " a",
  "a ",
  "a  b",
  "😀",
  "a😀b",
  "\ud83d",
  "ßİΣ",
  "a".repeat(31),
  "a".repeat(32),
  `${"😀".repeat(15)}a`,
  "😀".repeat(16),
  "abcde",
  "ab de",
  "漢字文字列",
  "猫である",
  "asearch",
  "toukyou",
  "haha",
])
  for (const mode of [0, 1, 2, 3, "1", null, undefined, NaN])
    add(pattern, mode, ["", pattern, `x${pattern}x`, "SSiσ", "\ud83d", "😀"]);
for (let i = 0; i < 3000; i++) {
  const pattern = i % 20 === 0 ? string(33, ["a", "漢", "😀", " "]) : string(8);
  add(pattern, [0, 1, 2][random(3)], [
    string(10),
    pattern,
    pattern.slice(0, -1),
  ]);
}
const metadata = {
  schema: 1,
  generator: "tools/fixtures/gen.mjs",
  locale,
  upstream_sha256: Object.fromEntries(
    [
      "./asearch.ts/lib/index.js",
      "./asearch.ts/src/index.ts",
      "./asearch/index.js",
    ].map((p) => [p, hashes[p]]),
  ),
  seed: 31,
  ambigs,
  node_ambigs: nodeAmbigs,
};
const json = `${JSON.stringify(metadata).slice(0, -1)},"cases":[\n${cases.map((c) => JSON.stringify(c)).join(",\n")}\n]}\n`;
if (Buffer.byteLength(json) > 4 * 1024 * 1024)
  throw new Error("Fixture exceeds 4 MiB");
const arg = process.argv.indexOf("--out");
const output = resolve(
  root,
  arg >= 0 ? process.argv[arg + 1] : "crates/asearch-rs/tests/fixtures",
  "compat.json",
);
if (process.argv.includes("--check")) {
  if ((await readFile(output, "utf8")) !== json)
    throw new Error("Fixtures are stale");
} else {
  await mkdir(resolve(output, ".."), { recursive: true });
  await writeFile(output, json);
}
console.log(
  `${cases.length} cases, ${Buffer.byteLength(json)} bytes, locale ${locale}`,
);
