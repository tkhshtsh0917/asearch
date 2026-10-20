import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFile, stat, writeFile } from "node:fs/promises";
import { arch, cpus, platform } from "node:os";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium, firefox, webkit } from "@playwright/test";
import { measureSessions } from "./session-measure.mjs";
import { startServer } from "./start-server.mjs";

const [indexPath, output, mode = "browser"] = process.argv.slice(2);
if (!indexPath || !output || !["browser", "node"].includes(mode))
  throw new Error(
    "Usage: node tools/bench/session.mjs INPUT_ASIX OUTPUT_JSON [browser|node]",
  );
const root = fileURLToPath(new URL("../../", import.meta.url));
if ((await stat(indexPath)).size > 256 * 1024 * 1024)
  throw new RangeError("Index exceeds 256 MiB");
const bytes = await readFile(indexPath);
const sha = (data) => createHash("sha256").update(data).digest("hex");
const results = [];
if (mode === "node") {
  const api = await import("../../packages/asearch-wasm/dist/node.mjs");
  results.push({
    name: "node",
    version: process.version,
    ...measureSessions(api, bytes),
  });
} else {
  const server = await startServer({ indexPath: resolve(indexPath) });
  try {
    for (const [name, type] of Object.entries({ chromium, firefox, webkit })) {
      const browser = await type.launch();
      try {
        const page = await browser.newPage();
        await page.goto(server.url);
        const report = await page.evaluate(async (expectedSha) => {
          const api = await import("/packages/asearch-wasm/dist/web.mjs");
          const { measureSessions } = await import(
            "/tools/bench/session-measure.mjs"
          );
          const response = await fetch("/fixture.asix");
          if (!response.ok) throw new Error("Index fixture request failed");
          const bytes = new Uint8Array(await response.arrayBuffer());
          const digest = new Uint8Array(
            await crypto.subtle.digest("SHA-256", bytes),
          );
          const actualSha = [...digest]
            .map((v) => v.toString(16).padStart(2, "0"))
            .join("");
          if (actualSha !== expectedSha)
            throw new Error("Fixture changed during benchmark");
          return measureSessions(api, bytes);
        }, sha(bytes));
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
  "tools/bench/session.mjs",
  "tools/bench/session-measure.mjs",
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
      corpusSha256: sha(bytes),
      bytes: bytes.length,
      artifacts,
      results,
    },
    null,
    2,
  )}\n`,
);
console.log(
  JSON.stringify(
    results.map(({ name, baseline, candidate, engines }) => ({
      name,
      baseline,
      candidate,
      engines,
    })),
  ),
);
