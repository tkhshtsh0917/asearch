import assert from "node:assert/strict";
import { createRequire } from "node:module";
import test from "node:test";
import * as esm from "../dist/node.mjs";

const cjs = createRequire(import.meta.url)("../dist/node.cjs");
const segment = (id = 1) => ({
  id: id.toString(16).padStart(16, "0"),
  url: `base-${id}.asix`,
  bytes: 64,
  texts: 0,
  integrity: `sha384-${"A".repeat(64)}`,
});
const manifest = (generation = 1, segments = [segment()]) => ({
  format: "asearch-manifest/1",
  indexId: "faq-v1",
  generation,
  index: {
    major: 1,
    minor: 0,
    symbol_unit: "code-point",
    normalizer: { id: "ja-basic", version: 1, options: 31 },
  },
  segments,
});
for (const [name, { diffManifest }] of [
  ["ESM", esm],
  ["CJS", cjs],
]) {
  test(`${name}: manifest generations distinguish append, replace, stale and identity`, () => {
    const current = manifest();
    assert.deepEqual(diffManifest(undefined, current), {
      kind: "replace",
      segments: current.segments,
    });
    assert.deepEqual(diffManifest(current, manifest()), { kind: "none" });
    assert.deepEqual(diffManifest(manifest(2), current), { kind: "stale" });
    assert.deepEqual(
      diffManifest(current, manifest(2, [segment(), segment(2)])),
      { kind: "append", add: [segment(2)] },
    );
    assert.deepEqual(diffManifest(current, manifest(2, [segment(2)])), {
      kind: "replace",
      segments: [segment(2)],
    });
    const moved = manifest(2);
    moved.segments[0].url = "https://cdn.example.test/new.asix";
    assert.deepEqual(diffManifest(current, moved), { kind: "none" });
    const reordered = JSON.parse(JSON.stringify(current), (key, value) =>
      key === "normalizer"
        ? { options: value.options, version: value.version, id: value.id }
        : value,
    );
    assert.deepEqual(diffManifest(current, reordered), { kind: "none" });
    const result = diffManifest(undefined, current);
    result.segments[0].url = "changed";
    assert.equal(current.segments[0].url, "base-1.asix");
  });
  test(`${name}: manifest conflicts cannot silently move series or redefine known segments`, () => {
    const current = manifest();
    for (const change of [
      (m) => {
        m.indexId = "another";
      },
      (m) => {
        m.index.symbol_unit = "utf16";
      },
      (m) => {
        m.index.normalizer.options = 0;
      },
      (m) => {
        m.segments[0].bytes++;
      },
      (m) => {
        m.segments[0].texts++;
      },
      (m) => {
        m.segments[0].integrity = `sha384-${"B".repeat(64)}`;
      },
    ]) {
      const candidate = manifest(2);
      change(candidate);
      assert.throws(() => diffManifest(current, candidate), {
        code: "CONFLICT",
      });
    }
    const sameGeneration = manifest();
    sameGeneration.segments[0].url = "moved.asix";
    assert.throws(() => diffManifest(current, sameGeneration), {
      code: "CONFLICT",
    });
    const stale = manifest(1);
    stale.indexId = "other";
    assert.throws(() => diffManifest(manifest(2), stale), { code: "CONFLICT" });
  });
  test(`${name}: manifest validates both inputs, quantities, descriptors and URL schemes`, () => {
    for (const change of [
      (m) => {
        m.format = "unknown";
      },
      (m) => {
        m.generation = 0;
      },
      (m) => {
        m.generation = Number.MAX_SAFE_INTEGER + 1;
      },
      (m) => {
        m.indexId = "bad id";
      },
      (m) => {
        m.index.major = 2;
      },
      (m) => {
        m.index.normalizer.version = 2;
      },
      (m) => {
        m.index.normalizer.options = 64;
      },
      (m) => {
        m.segments = [];
      },
      (m) => {
        m.segments = Array.from({ length: 9 }, (_, i) => segment(i));
      },
      (m) => {
        m.segments.push(segment());
      },
      (m) => {
        m.segments[0].id = "0123456789ABCDEF";
      },
      (m) => {
        m.segments[0].bytes = 63;
      },
      (m) => {
        m.segments[0].texts = -1;
      },
      (m) => {
        m.segments[0].integrity = "sha384-x";
      },
      ...[
        "",
        "javascript:alert(1)",
        "file:///index",
        "data:text/plain,x",
        "https://u:p@example.test/i",
        "//u@example.test/i",
        " a.asix",
        "a\\b",
        "https://[",
      ].map((url) => (m) => {
        m.segments[0].url = url;
      }),
    ]) {
      const candidate = manifest();
      change(candidate);
      assert.throws(() => diffManifest(undefined, candidate), {
        code: "INVALID_MANIFEST",
      });
      assert.throws(() => diffManifest(candidate, manifest()), {
        code: "INVALID_MANIFEST",
      });
    }
    const maximum = manifest(Number.MAX_SAFE_INTEGER);
    assert.equal(diffManifest(manifest(), maximum).kind, "none");
    const input = [segment()];
    input[Symbol.iterator] = function* () {
      yield segment(2);
    };
    assert.deepEqual(diffManifest(undefined, manifest(1, input)).segments, [
      segment(),
    ]);
  });
}
