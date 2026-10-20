import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import * as esm from "../dist/node.mjs";

const cjs = createRequire(import.meta.url)("../dist/node.cjs");
const root = fileURLToPath(new URL("../../../", import.meta.url));
function tombstones(bytes) {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  for (let i = 0; i < view.getUint32(12, true); i++) {
    const at = 64 + 16 * i;
    if (view.getUint32(at, true) === 0x424d4f54) {
      const offset = view.getUint32(at + 8, true),
        length = view.getUint32(at + 12, true);
      return Array.from({ length: length / 4 }, (_, j) =>
        view.getUint32(offset + 4 * j, true),
      );
    }
  }
  return [];
}
for (const [name, api] of [
  ["ESM", esm],
  ["CJS", cjs],
]) {
  test(`${name}: immutable server deltas preserve replacement texts and sorted tombstones`, () => {
    const docs = [{ key: 2, order: 3, texts: ["ｶﾞと新しい本文"] }];
    const bytes = api.buildDelta(docs, [9, 2, 1]);
    assert.deepEqual(tombstones(bytes), [1, 2, 9]);
    assert.deepEqual(bytes, api.buildDelta(docs, [1, 9, 2]));
    const index = api.AsearchIndex.fromBytes(bytes);
    try {
      assert.deepEqual([...index.search("本文", { maxAmbig: 0 }).ids], [2]);
      assert.deepEqual(index.toBytes(), api.buildIndex(docs));
    } finally {
      index.free();
    }
    assert.deepEqual(tombstones(api.buildDelta([], [9])), [9]);
    assert.deepEqual(api.buildDelta(docs, []), api.buildIndex(docs));
    for (const keys of [[1, 1], [-1], [2 ** 32], [null], [1.5]])
      assert.throws(() => api.buildDelta(docs, keys));
    assert.throws(
      () => api.buildDelta([], [1, 2], { limits: { maxTotalBytes: 4 } }),
      { code: "RESOURCE_LIMIT" },
    );
  });
}
test("CLI delta, compaction and manifest agree with WASM and trusted loaders", async (t) => {
  const temporary = await mkdtemp(join(tmpdir(), "asearch-delta-"));
  t.after(() => rm(temporary, { recursive: true, force: true }));
  const input = join(temporary, "changes.jsonl"),
    output = join(temporary, "delta.asix");
  const doc = { key: 2, order: 3, texts: ["ｶﾞと新しい本文", "another"] };
  await writeFile(
    input,
    `${JSON.stringify({ op: "upsert", ...doc })}\n${JSON.stringify({ op: "remove", key: 9 })}\n`,
  );
  const build = spawnSync(
    "cargo",
    ["build", "--locked", "--release", "-p", "asearch-cli"],
    { cwd: root, encoding: "utf8" },
  );
  assert.equal(build.status, 0, build.stderr);
  const cli = spawnSync(
    join(root, "target/release/asearch-cli"),
    ["delta", input, output],
    { cwd: root, encoding: "utf8" },
  );
  assert.equal(cli.status, 0, cli.stderr);
  assert.deepEqual(
    new Uint8Array(await readFile(output)),
    esm.buildDelta([doc], [9, 2]),
  );
  const baseFile = join(temporary, "base.asix"),
    compactFile = join(temporary, "compact.asix"),
    manifestFile = join(temporary, "manifest.json");
  const original = { key: 9, texts: ["removed"] };
  const base = esm.buildIndex([original]);
  await writeFile(baseFile, base);
  const compact = spawnSync(
    join(root, "target/release/asearch-cli"),
    ["compact", compactFile, baseFile, output],
    { encoding: "utf8" },
  );
  assert.equal(compact.status, 0, compact.stderr);
  assert.deepEqual(
    new Uint8Array(await readFile(compactFile)),
    esm.buildIndex([doc]),
  );
  const publication = spawnSync(
    join(root, "target/release/asearch-cli"),
    [
      "manifest",
      "faq",
      "2",
      manifestFile,
      `base.asix=${baseFile}`,
      `delta.asix=${output}`,
    ],
    { encoding: "utf8" },
  );
  assert.equal(publication.status, 0, publication.stderr);
  const text = await readFile(manifestFile, "utf8"),
    manifest = JSON.parse(text);
  assert.equal(text.includes(temporary), false);
  assert.equal(esm.diffManifest(undefined, manifest).kind, "replace");
  const index = esm.AsearchIndex.fromBytes(base, {
    expected: manifest.segments[0],
  });
  try {
    await index.addSegment(
      new Uint8Array(await readFile(output)),
      manifest.segments[1],
    );
    assert.deepEqual(
      index.toBytes(),
      new Uint8Array(await readFile(compactFile)),
    );
  } finally {
    index.free();
  }
});
