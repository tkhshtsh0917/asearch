// Benchmark-only loopback server. No application or deployment responsibilities.
import { readFile, stat } from "node:fs/promises";
import { createServer } from "node:http";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));
const server = createServer(async (req, res) => {
  const path = new URL(req.url, "http://127.0.0.1").pathname;
  if (path === "/") {
    res
      .writeHead(200, { "Content-Type": "text/html" })
      .end("<!doctype html><title>Asearch benchmark</title>");
    return;
  }
  if (path === "/fixture.asix" && process.env.ASEARCH_BENCH_INDEX) {
    try {
      if (
        (await stat(process.env.ASEARCH_BENCH_INDEX)).size >
        256 * 1024 * 1024
      )
        throw new RangeError("Fixture too large");
      const body = await readFile(process.env.ASEARCH_BENCH_INDEX);
      res
        .writeHead(200, {
          "Content-Type": "application/octet-stream",
          "Cache-Control": "no-store",
        })
        .end(body);
    } catch {
      res.writeHead(404).end();
    }
    return;
  }
  if (
    !/^\/(tools\/bench\/[a-z-]+\.mjs|packages\/asearch-wasm\/dist\/[a-zA-Z0-9_/.]+|vendor\/upstream\/asearch.ts\/lib\/index.js)$/.test(
      path,
    ) ||
    path.includes("..") ||
    path.includes("%")
  ) {
    res.writeHead(404).end();
    return;
  }
  try {
    const body = await readFile(root + path.slice(1));
    res
      .writeHead(200, {
        "Content-Type": path.endsWith(".wasm")
          ? "application/wasm"
          : "text/javascript",
        "Cache-Control": "no-store",
      })
      .end(body);
  } catch {
    res.writeHead(404).end();
  }
});
server.listen(0, "127.0.0.1", () => {
  process.send?.({ port: server.address().port });
});
