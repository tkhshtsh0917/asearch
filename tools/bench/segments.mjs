import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import { arch, cpus, platform } from "node:os";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium, firefox, webkit } from "@playwright/test";
import { measureSegments } from "./segments-measure.mjs";
import { startServer } from "./start-server.mjs";

const [output, mode = "browser"] = process.argv.slice(2);
if (!output || !["browser", "node"].includes(mode))
  throw new Error(
    "Usage: node tools/bench/segments.mjs OUTPUT_JSON [browser|node]",
  );
const root = fileURLToPath(new URL("../../", import.meta.url));
const sha = (data) => createHash("sha256").update(data).digest("hex");
const results = [];
if (mode === "node") {
  const api = await import("../../packages/asearch-wasm/dist/node.mjs");
  const { default: bindings } = await import(
    "../../packages/asearch-wasm/dist/node/asearch_wasm.js"
  );
  results.push({
    name: "node",
    version: process.version,
    ...(await measureSegments(
      api,
      {},
      () => bindings.index_memory().buffer.byteLength,
    )),
  });
} else {
  const server = await startServer();
  try {
    for (const [name, type] of Object.entries({ chromium, firefox, webkit })) {
      const browser = await type.launch();
      try {
        const page = await browser.newPage();
        await page.goto(server.url);
        const report = await page.evaluate(async () => {
          const api = await import("/packages/asearch-wasm/dist/web.mjs");
          const { measureSegments } = await import(
            "/tools/bench/segments-measure.mjs"
          );
          const bindings = await import(
            "/packages/asearch-wasm/dist/web/asearch_wasm.js"
          );
          return measureSegments(
            api,
            {},
            () => bindings.index_memory().buffer.byteLength,
          );
        });
        results.push({ name, version: browser.version(), ...report });
      } finally {
        await browser.close();
      }
    }
  } finally {
    await server.close();
  }
}
const artifacts = {};
for (const path of [
  "packages/asearch-wasm/dist/web/asearch_wasm_bg.wasm",
  "packages/asearch-wasm/dist/web.mjs",
  "packages/asearch-wasm/dist/node/asearch_wasm_bg.wasm",
  "packages/asearch-wasm/dist/node.mjs",
  "tools/bench/segments.mjs",
  "tools/bench/segments-measure.mjs",
  "tools/bench/segments-lifecycle.mjs",
  "pnpm-lock.yaml",
])
  artifacts[path] = sha(await readFile(resolve(root, path)));
await writeFile(
  output,
  `${JSON.stringify(
    {
      recordedAtUtc: new Date().toISOString(),
      commit: execFileSync("git", ["rev-parse", "HEAD"], {
        cwd: root,
        encoding: "utf8",
      }).trim(),
      dirty: Boolean(
        execFileSync("git", ["status", "--porcelain"], {
          cwd: root,
          encoding: "utf8",
        }).trim(),
      ),
      os: platform(),
      arch: arch(),
      cpu: cpus()[0]?.model,
      artifacts,
      results,
    },
    null,
    2,
  )}\n`,
);
console.log(
  JSON.stringify(
    results.map(({ name, reports }) => ({
      name,
      reports: reports.map(({ segments, baseline, candidate, lifecycle }) => ({
        segments,
        baseline,
        candidate,
        lifecycle: {
          load: lifecycle.load,
          compact: lifecycle.compact,
          update: lifecycle.update,
        },
      })),
    })),
  ),
);
