// Compare built distributions, not timing files collected in different sessions.
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import { arch, cpus, platform } from "node:os";
import { resolve } from "node:path";
import { performance } from "node:perf_hooks";
import { fileURLToPath, pathToFileURL } from "node:url";
import { measure } from "./measure.mjs";
import { regression } from "./statistics.mjs";
import { workload } from "./workload.mjs";

const [
  baselineDist,
  candidateDist,
  output,
  baselineCommit,
  candidateCommit,
  units = "10000",
] = process.argv.slice(2);
if (
  !output ||
  !/^[a-f0-9]{40}$/.test(baselineCommit ?? "") ||
  !/^[a-f0-9]{40}$/.test(candidateCommit ?? "")
)
  throw new Error(
    "Usage: node tools/bench/compare.mjs BASELINE_DIST CANDIDATE_DIST OUTPUT_JSON BASELINE_COMMIT CANDIDATE_COMMIT [UNITS] (full commit IDs)",
  );

const sha = (bytes) => createHash("sha256").update(bytes).digest("hex");
async function load(directory, commit) {
  const root = pathToFileURL(`${resolve(directory)}/`);
  const artifacts = {};
  for (const path of [
    "node.mjs",
    "node/asearch_wasm.js",
    "node/asearch_wasm_bg.wasm",
  ])
    artifacts[path] = sha(await readFile(new URL(path, root)));
  const { Asearch } = await import(new URL("node.mjs", root));
  return { Asearch, identity: { commit, artifacts } };
}

const baseline = await load(baselineDist, baselineCommit);
const candidate = await load(candidateDist, candidateCommit);
const data = workload(Number(units));
const now = () => performance.now();
const first = measure(baseline.Asearch, candidate.Asearch, data, now);
// Do not select only favorable cases. A candidate in any pattern triggers a
// second complete independent group, preserving the corpus and AB/BA schedule.
const needsConfirmation = first.some(
  ({ runs }) => regression(runs).status === "candidate",
);
const confirmation = needsConfirmation
  ? measure(baseline.Asearch, candidate.Asearch, data, now)
  : undefined;
const report = {
  category:
    "A: before/after fixed-level match, batch mean per match; not keystroke latency",
  profile: "asearch.ts-lib",
  mode: "Include",
  level: 3,
  timer: "performance.now (milliseconds)",
  recordedAtUtc: new Date().toISOString(),
  harnessCommit: execFileSync("git", ["rev-parse", "HEAD"], {
    cwd: fileURLToPath(new URL("../../", import.meta.url)),
    encoding: "utf8",
  }).trim(),
  harnessDirty:
    execFileSync("git", ["status", "--porcelain"], {
      cwd: fileURLToPath(new URL("../../", import.meta.url)),
      encoding: "utf8",
    }).trim().length > 0,
  node: process.version,
  v8: process.versions.v8,
  rustRunningHarness: execFileSync("rustc", ["--version"], {
    cwd: fileURLToPath(new URL("../../", import.meta.url)),
    encoding: "utf8",
  }).trim(),
  os: platform(),
  arch: arch(),
  cpu: cpus()[0]?.model,
  baseline: baseline.identity,
  candidate: candidate.identity,
  runnerSha256: sha(await readFile(new URL("./compare.mjs", import.meta.url))),
  harnessSha256: sha(await readFile(new URL("./measure.mjs", import.meta.url))),
  statisticsSha256: sha(
    await readFile(new URL("./statistics.mjs", import.meta.url)),
  ),
  corpusSha256: sha(JSON.stringify(data)),
  seed: data.seed,
  units: data.units,
  first,
  confirmation,
  decisions: first.map(({ pattern, runs }, i) => ({
    pattern,
    ...regression(runs, confirmation?.[i].runs),
  })),
};
await writeFile(output, `${JSON.stringify(report, null, 2)}\n`);
console.log(
  JSON.stringify({
    outputWritten: true,
    units: data.units,
    decisions: report.decisions,
  }),
);
