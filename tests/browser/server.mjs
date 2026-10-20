// Loopback-only test server. Not a deployment server.

import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));
// Serve a freshly packed distribution independently of the source dist tree.
const packed = mkdtempSync(join(tmpdir(), "asearch-browser-pack-"));
function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: root,
    encoding: "utf8",
    env: {
      ...process.env,
      npm_config_cache: join(tmpdir(), "asearch-m0-npm-cache"),
    },
  });
  if (result.status !== 0) throw new Error(result.stderr || result.stdout);
  return result.stdout;
}
const [artifact] = JSON.parse(
  run("npm", [
    "pack",
    "--workspace",
    "packages/asearch-wasm",
    "--json",
    "--pack-destination",
    packed,
  ]),
);
run("tar", ["-xzf", join(packed, artifact.filename), "-C", packed]);
process.on("exit", () => rmSync(packed, { recursive: true, force: true }));
for (const signal of ["SIGTERM", "SIGINT"])
  process.on(signal, () => process.exit(0));

const types = {
  wasm: "application/wasm",
  mjs: "text/javascript",
  js: "text/javascript",
  json: "application/json",
  html: "text/html",
};
const server = createServer(async (req, res) => {
  try {
    const path = new URL(req.url, "http://127.0.0.1").pathname;
    if (
      !/^\/(packed\/dist\/|packages\/asearch-wasm\/dist\/|tests\/browser\/fixtures\/|crates\/asearch-rs\/tests\/fixtures\/compat.json$)/.test(
        path,
      ) ||
      path.includes("%") ||
      path.includes("\\")
    ) {
      res.writeHead(404).end();
      return;
    }
    const body = await readFile(
      path.startsWith("/packed/")
        ? join(packed, "package", path.slice("/packed/".length))
        : root + path.slice(1),
    );
    res.writeHead(200, {
      "Content-Type":
        types[path.split(".").at(-1)] ?? "application/octet-stream",
      "Cache-Control": "no-store",
    });
    res.end(body);
  } catch {
    res.writeHead(404).end();
  }
});
server.listen(4387, "127.0.0.1");
