import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import { arch, cpus, platform } from "node:os";
import { performance } from "node:perf_hooks";
import { fileURLToPath } from "node:url";
import vm from "node:vm";
import { Asearch } from "../../packages/asearch-wasm/dist/node.mjs";
import { extensions } from "./extensions.mjs";
import { measure } from "./measure.mjs";
import { JsReplica } from "./replica.mjs";
import { workload } from "./workload.mjs";

const root = fileURLToPath(new URL("../../", import.meta.url));
const [output, units = "10000"] = process.argv.slice(2);
if (!output)
  throw new Error("Usage: node tools/bench/node.mjs OUTPUT_JSON [UNITS]");
const source = await readFile(
  new URL("../../vendor/upstream/asearch.ts/lib/index.js", import.meta.url),
  "utf8",
);
const module = { exports: {} };
vm.runInNewContext(source, {
  module,
  exports: module.exports,
  process: { env: {} },
});
const data = workload(Number(units));
const sha = (bytes) => createHash("sha256").update(bytes).digest("hex");
const harnessSha256 = {};
for (const name of [
  "node",
  "measure",
  "replica",
  "extensions",
  "statistics",
  "workload",
])
  harnessSha256[name] = sha(
    await readFile(new URL(`./${name}.mjs`, import.meta.url)),
  );
const report = {
  category:
    "A: fixed-level match, batch mean per match; not keystroke percentiles",
  profile: "asearch.ts-lib",
  mode: "Include",
  level: 3,
  commit: execFileSync("git", ["rev-parse", "HEAD"], {
    cwd: root,
    encoding: "utf8",
  }).trim(),
  worktreeDirty:
    execFileSync("git", ["status", "--porcelain"], {
      cwd: root,
      encoding: "utf8",
    }).trim().length > 0,
  node: process.version,
  v8: process.versions.v8,
  os: platform(),
  arch: arch(),
  cpu: cpus()[0]?.model,
  rust: execFileSync("rustc", ["--version"], {
    cwd: root,
    encoding: "utf8",
  }).trim(),
  recordedAtUtc: new Date().toISOString(),
  wasmSha256: sha(
    await readFile(
      new URL(
        "../../packages/asearch-wasm/dist/node/asearch_wasm_bg.wasm",
        import.meta.url,
      ),
    ),
  ),
  wrapperSha256: sha(
    await readFile(
      new URL("../../packages/asearch-wasm/dist/node.mjs", import.meta.url),
    ),
  ),
  lockSha256: sha(
    await readFile(new URL("../../pnpm-lock.yaml", import.meta.url)),
  ),
  sourceSha256: sha(source),
  harnessSha256,
  corpusSha256: sha(JSON.stringify(data)),
  units: data.units,
  seed: data.seed,
  results: measure(module.exports.Asearch, Asearch, data, () =>
    performance.now(),
  ),
  replicaResults: measure(JsReplica, Asearch, data, () => performance.now()),
  extensions: extensions(JsReplica, Asearch, data, () => performance.now()),
};
await writeFile(output, `${JSON.stringify(report, null, 2)}\n`);
console.log(
  JSON.stringify({
    units: report.units,
    patterns: report.results.length,
    comparison: "upstream JS vs Rust/WASM wrapper",
    outputWritten: true,
  }),
);
