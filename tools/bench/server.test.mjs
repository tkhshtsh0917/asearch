import assert from "node:assert/strict";
import { createServer } from "node:http";
import test from "node:test";
import { startServer } from "./start-server.mjs";

test("parallel benchmark servers identify their own listening endpoints", async () => {
  const unrelated = createServer((_, res) => res.end("unrelated"));
  await new Promise((resolve) => unrelated.listen(0, "127.0.0.1", resolve));
  const servers = [];
  try {
    servers.push(await startServer(), await startServer());
    assert.notEqual(servers[0].url, servers[1].url);
    for (const server of servers) {
      assert.notEqual(
        new URL(server.url).port,
        String(unrelated.address().port),
      );
      assert.match(await (await fetch(server.url)).text(), /Asearch benchmark/);
    }
  } finally {
    for (const server of servers) await server.close();
    await new Promise((resolve) => unrelated.close(resolve));
  }
});
