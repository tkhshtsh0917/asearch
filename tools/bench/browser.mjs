import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import { arch, cpus, platform } from "node:os";
import { fileURLToPath } from "node:url";
import { chromium, firefox, webkit } from "@playwright/test";
import { startServer } from "./start-server.mjs";

const root = fileURLToPath(new URL("../../", import.meta.url));
const [output, units = "10000"] = process.argv.slice(2);
if (!output)
  throw new Error("Usage: node tools/bench/browser.mjs OUTPUT_JSON [UNITS]");
const server = await startServer();
const sha = (bytes) => createHash("sha256").update(bytes).digest("hex");
const reports = [];
try {
  // Measure sequentially; browser engines must not compete for CPU.
  for (const [name, type] of Object.entries({ chromium, firefox, webkit })) {
    const browser = await type.launch({ headless: true });
    try {
      const page = await browser.newPage();
      await page.goto(server.url);
      const result = await page.evaluate(async (units) => {
        const { Asearch } = await import("/packages/asearch-wasm/dist/web.mjs");
        const { workload } = await import("/tools/bench/workload.mjs");
        const { measure } = await import("/tools/bench/measure.mjs");
        const { extensions } = await import("/tools/bench/extensions.mjs");
        const { JsReplica } = await import("/tools/bench/replica.mjs");
        const source = await (
          await fetch("/vendor/upstream/asearch.ts/lib/index.js")
        ).text();
        const module = { exports: {} };
        new Function("module", "exports", "process", source)(
          module,
          module.exports,
          { env: {} },
        );
        const data = workload(units);
        return {
          corpus: JSON.stringify(data),
          userAgent: navigator.userAgent,
          results: measure(module.exports.Asearch, Asearch, data, () =>
            performance.now(),
          ),
          replicaResults: measure(JsReplica, Asearch, data, () =>
            performance.now(),
          ),
          extensions: extensions(JsReplica, Asearch, data, () =>
            performance.now(),
          ),
        };
      }, Number(units));
      const { corpus, ...data } = result;
      reports.push({
        name,
        version: browser.version(),
        corpusSha256: sha(corpus),
        ...data,
      });
    } finally {
      await browser.close();
    }
  }
  const artifacts = {};
  for (const path of [
    "packages/asearch-wasm/dist/web/asearch_wasm_bg.wasm",
    "packages/asearch-wasm/dist/web.mjs",
    "vendor/upstream/asearch.ts/lib/index.js",
    "pnpm-lock.yaml",
    "tools/bench/replica.mjs",
    "tools/bench/extensions.mjs",
    "tools/bench/measure.mjs",
    "tools/bench/statistics.mjs",
    "tools/bench/workload.mjs",
  ])
    artifacts[path] = sha(
      await readFile(new URL(`../../${path}`, import.meta.url)),
    );
  const report = {
    category: "A: batch mean per fixed-level match; not keystroke percentiles",
    profile: "asearch.ts-lib",
    mode: "Include",
    level: 3,
    seed: 31,
    units: Number(units),
    recordedAtUtc: new Date().toISOString(),
    commit: execFileSync("git", ["rev-parse", "HEAD"], {
      cwd: root,
      encoding: "utf8",
    }).trim(),
    worktreeDirty:
      execFileSync("git", ["status", "--porcelain"], {
        cwd: root,
        encoding: "utf8",
      }).trim().length > 0,
    rust: execFileSync("rustc", ["--version"], {
      cwd: root,
      encoding: "utf8",
    }).trim(),
    os: platform(),
    arch: arch(),
    cpu: cpus()[0]?.model,
    artifacts,
    reports,
  };
  await writeFile(output, `${JSON.stringify(report, null, 2)}\n`);
  console.log(
    JSON.stringify({
      browsers: reports.map((r) => r.name),
      units: report.units,
      outputWritten: true,
    }),
  );
} finally {
  await server.close();
}
