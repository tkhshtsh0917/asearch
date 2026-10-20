import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { arch, cpus, platform, tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { pairedInterval, quantile } from "./statistics.mjs";
import { workload } from "./workload.mjs";

const [output, units = "10000"] = process.argv.slice(2);
if (!output)
  throw new Error("Usage: node tools/bench/native.mjs OUTPUT_JSON [UNITS]");
const root = fileURLToPath(new URL("../../", import.meta.url));
const run = (command, args) =>
  execFileSync(command, args, { cwd: root, encoding: "utf8" });
const data = workload(Number(units));
const sha = (bytes) => createHash("sha256").update(bytes).digest("hex");
const directory = await mkdtemp(join(tmpdir(), "asearch-native-bench-"));
try {
  execFileSync(
    "cargo",
    ["build", "--locked", "--release", "-p", "asearch-cli"],
    { cwd: root, stdio: "inherit" },
  );
  const metadata = JSON.parse(
    run("cargo", [
      "metadata",
      "--locked",
      "--no-deps",
      "--format-version",
      "1",
    ]),
  );
  const binary = join(
    metadata.target_directory,
    "release",
    platform() === "win32" ? "asearch-cli.exe" : "asearch-cli",
  );
  const input = join(directory, "input.json");
  const raw = join(directory, "raw.json");
  await writeFile(input, JSON.stringify(data));
  run(binary, ["bench", input, raw]);
  const report = JSON.parse(await readFile(raw, "utf8"));
  for (const result of report.results) {
    const { runs } = result.measurement;
    result.interval = pairedInterval(runs);
    result.summary = Object.fromEntries(
      ["baseline", "candidate"].map((name) => {
        const samples = runs.flatMap((run) => run[name]);
        return [
          name,
          {
            batchMeanMatchMsP50: quantile(samples, 0.5),
            batchMeanMatchMsP95: quantile(samples, 0.95),
            batchMeanMatchMsP99: quantile(samples, 0.99),
          },
        ];
      }),
    );
  }
  Object.assign(report, {
    recordedAtUtc: new Date().toISOString(),
    commit: run("git", ["rev-parse", "HEAD"]).trim(),
    worktreeDirty: run("git", ["status", "--porcelain"]).trim().length > 0,
    rust: run("rustc", ["--version"]).trim(),
    node: process.version,
    cpu: cpus()[0]?.model,
    os: platform(),
    arch: arch(),
    corpusSha256: sha(JSON.stringify(data)),
    binarySha256: sha(await readFile(binary)),
    cargoLockSha256: sha(
      await readFile(new URL("../../Cargo.lock", import.meta.url)),
    ),
    statisticsSha256: sha(
      await readFile(new URL("./statistics.mjs", import.meta.url)),
    ),
    runnerSha256: sha(await readFile(new URL("./native.mjs", import.meta.url))),
  });
  await writeFile(output, `${JSON.stringify(report, null, 2)}\n`);
  console.log(
    JSON.stringify({
      outputWritten: true,
      units: report.units,
      cases: report.results.length,
    }),
  );
} finally {
  await rm(directory, { recursive: true, force: true });
}
