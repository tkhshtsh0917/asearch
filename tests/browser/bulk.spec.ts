import { expect, test } from "@playwright/test";

test("bulk index builds, loads and searches without Node globals", async ({
  page,
}) => {
  await page.goto("/tests/browser/fixtures/index.html");
  const result = await page.evaluate(async () => {
    // @ts-expect-error Browser-only absolute module URL.
    const api = await import("/packages/asearch-wasm/dist/web.mjs");
    const index = api.AsearchIndex.fromDocs(
      [
        { key: 5, texts: ["ｶﾞの使い方"] },
        { key: 2, texts: ["ガの使い方", "別の質問"] },
      ],
      { limits: { maxPatternUnits: 64 } },
    );
    try {
      const found = index
        .search("がの使い方", { countTotal: true, maxAmbig: api.fixed(0) })
        .copy();
      const bytes = index.toBytes();
      let position = 0;
      const body = new ReadableStream({
        pull(controller) {
          if (position === bytes.length) {
            controller.close();
            return;
          }
          controller.enqueue(bytes.subarray(position, position + 17));
          position = Math.min(bytes.length, position + 17);
        },
      });
      const loaded = await api.AsearchIndex.fromStream(body, {
        limits: { maxPatternUnits: 64 },
      });
      try {
        return {
          ids: [...found.ids],
          totals: [...found.totals],
          loaded: [...loaded.search("がの使い方", { maxAmbig: 0 }).ids],
          blocks: found.stats.blocks,
        };
      } finally {
        loaded.free();
      }
    } finally {
      index.free();
    }
  });
  expect(result.ids).toEqual([2, 5]);
  expect(result.loaded).toEqual([2, 5]);
  expect(result.totals).toEqual([2]);
  expect(result.blocks).toBeGreaterThan(0);
});

test("bulk profiles preserve host-locale casing and UTF-16 text differences", async ({
  page,
}) => {
  await page.goto("/tests/browser/fixtures/index.html");
  const result = await page.evaluate(async () => {
    // @ts-expect-error Browser-only absolute module URL.
    const api = await import("/packages/asearch-wasm/dist/web.mjs");
    const lower = String.prototype.toLocaleLowerCase;
    const rows = [];
    try {
      String.prototype.toLocaleLowerCase = function () {
        return lower.call(this, "tr");
      };
      for (const profile of ["ideal", "asearch.ts", "asearch.ts-lib"]) {
        const index = api.AsearchIndex.fromStrings(["I", "i", "ı", "😀"], {
          profile,
          normalizer: "none",
        });
        try {
          rows.push({
            profile,
            casing: [...index.search("I", { mode: 0, maxAmbig: 0 }).ids],
            emoji: [...index.search("😀", { mode: 0, maxAmbig: 0 }).ids],
          });
        } finally {
          index.free();
        }
      }
    } finally {
      String.prototype.toLocaleLowerCase = lower;
    }
    return rows;
  });
  expect(result).toEqual([
    { profile: "ideal", casing: [0, 2], emoji: [3] },
    { profile: "asearch.ts", casing: [0, 2], emoji: [3] },
    { profile: "asearch.ts-lib", casing: [0, 2], emoji: [] },
  ]);
});

test("sessions reuse normalized prefixes and release with the parent index", async ({
  page,
}) => {
  await page.goto("/tests/browser/fixtures/index.html");
  const result = await page.evaluate(async () => {
    // @ts-expect-error Browser-only absolute module URL.
    const api = await import("/packages/asearch-wasm/dist/web.mjs");
    const index = api.AsearchIndex.fromStrings(["abcdef", "がく", "😀abcd"], {
      limits: { maxPatternUnits: 64 },
    });
    const session = index.session({ countTotal: true });
    const rows = [];
    try {
      for (const pattern of [
        "a",
        "ab",
        "abc",
        "abcd",
        "abc",
        "ｶ",
        "ｶﾞ",
        "ｶﾞｸ",
        "😀",
        "😀a",
        "😀ab",
      ]) {
        const actual = session.update(pattern);
        const expected = index.search(pattern, { countTotal: true });
        rows.push({
          same:
            String(actual.ids) === String(expected.ids) &&
            String(actual.levels) === String(expected.levels) &&
            String(actual.totals) === String(expected.totals),
          reused: actual.stats.reusedRows,
          narrowed: actual.stats.engine === "narrowing",
        });
      }
      session.reset();
      const resetRows = session.update("abc").stats.reusedRows;
      index.free();
      let disposed = "";
      try {
        session.update("a");
      } catch (error) {
        disposed = error.code;
      }
      return { rows, resetRows, disposed };
    } finally {
      session.free();
      index.free();
    }
  });
  expect(result.rows.every((row: { same: boolean }) => row.same)).toBe(true);
  expect(
    result.rows.some(
      (row: { reused: number; narrowed: boolean }) =>
        row.reused > 0 || row.narrowed,
    ),
  ).toBe(true);
  expect(result.resetRows).toBe(0);
  expect(result.disposed).toBe("DISPOSED");
});

test("trusted manifest hashes protect byte and stream loads", async ({
  page,
}) => {
  await page.goto("/tests/browser/fixtures/index.html");
  const result = await page.evaluate(async () => {
    // @ts-expect-error Browser-only absolute module URL.
    const api = await import("/packages/asearch-wasm/dist/web.mjs");
    const bytes = api.buildDelta([{ key: 1, texts: ["検索"] }], [1]);
    const digest = new Uint8Array(await crypto.subtle.digest("SHA-384", bytes));
    const expected = {
      id: new DataView(bytes.buffer, bytes.byteOffset)
        .getBigUint64(24, true)
        .toString(16)
        .padStart(16, "0"),
      url: "base.asix",
      bytes: bytes.length,
      texts: 1,
      integrity: `sha384-${btoa(String.fromCharCode(...digest))}`,
    };
    const manifest = {
      format: "asearch-manifest/1",
      indexId: "test",
      generation: 1,
      index: {
        major: 1,
        minor: 0,
        symbol_unit: "code-point",
        normalizer: { id: "ja-basic", version: 1, options: 31 },
      },
      segments: [expected],
    };
    const change = api.diffManifest(undefined, manifest);
    const body = new ReadableStream({
      start(controller) {
        controller.enqueue(bytes);
        controller.close();
      },
    });
    const index = await api.AsearchIndex.fromStream(body, { expected });
    try {
      const ids = [...index.search("検索").ids];
      let rejected = "";
      try {
        api.AsearchIndex.fromBytes(bytes, {
          expected: { ...expected, integrity: `sha384-${"A".repeat(64)}` },
        });
      } catch (error) {
        rejected = error.code;
      }
      return { ids, rejected, kind: change.kind, locked: body.locked };
    } finally {
      index.free();
    }
  });
  expect(result).toEqual({
    ids: [1],
    rejected: "INVALID_INDEX",
    kind: "replace",
    locked: false,
  });
});

test("streamed segment adoption remains atomic with active sessions", async ({
  page,
}) => {
  await page.goto("/tests/browser/fixtures/index.html");
  const result = await page.evaluate(async () => {
    // @ts-expect-error Browser-only absolute module URL.
    const api = await import("/packages/asearch-wasm/dist/web.mjs");
    const index = api.AsearchIndex.fromStrings(["old"]),
      session = index.session();
    const delta = api.buildDelta([{ key: 0, texts: ["new"] }], [0]);
    const view = new DataView(delta.buffer, delta.byteOffset, delta.byteLength);
    const digest = new Uint8Array(await crypto.subtle.digest("SHA-384", delta));
    const expected = {
      id: view.getBigUint64(24, true).toString(16).padStart(16, "0"),
      url: "delta.asix",
      bytes: delta.length,
      texts: 1,
      integrity: `sha384-${btoa(String.fromCharCode(...digest))}`,
    };
    let send: ReadableStreamDefaultController<Uint8Array> | undefined;
    const body = new ReadableStream<Uint8Array>({
      start(controller) {
        send = controller;
        controller.enqueue(delta.subarray(0, 64));
      },
    });
    try {
      const before = index.toBytes(),
        ref = index.search("old").refs[0];
      const pending = index.addSegment(body, expected);
      await Promise.resolve();
      const old = [...session.update("old").ids];
      let busy = "";
      try {
        await index.addSegment(delta);
      } catch (error) {
        busy = (error as { code: string }).code;
      }
      send?.enqueue(delta.subarray(64));
      send?.close();
      await pending;
      const current = session.update("new");
      let failure = "";
      try {
        await index.addSegment(new Uint8Array(64));
      } catch (error) {
        failure = (error as { code: string }).code;
      }
      const compact = index.toBytes(),
        canonical = api.buildIndex([{ key: 0, texts: ["new"] }]);
      await index.replaceSegments([{ source: before }]);
      return {
        old,
        busy,
        failure,
        current: [...current.ids],
        segments: current.stats.segments,
        refChanged: ref !== current.refs[0],
        compactEqual: String(compact) === String(canonical),
        restored: [...session.update("old").ids],
        unlocked: !body.locked,
      };
    } finally {
      session.free();
      index.free();
    }
  });
  expect(result).toEqual({
    old: [0],
    busy: "BUSY",
    failure: "INVALID_INDEX",
    current: [0],
    segments: 2,
    refChanged: true,
    compactEqual: true,
    restored: [0],
    unlocked: true,
  });
});

test("explain maps composed kana and expires copied refs on successful updates", async ({
  page,
}) => {
  await page.goto("/tests/browser/fixtures/index.html");
  const result = await page.evaluate(async () => {
    // @ts-expect-error Browser-only absolute module URL.
    const api = await import("/packages/asearch-wasm/dist/web.mjs");
    const index = api.AsearchIndex.fromStrings(["ｶﾞ😀"]);
    try {
      const found = index.search("が😀", { maxAmbig: 0 }).copy();
      const explained = index.explain(found.refs[0], "が😀", { maxAmbig: 0 });
      await index.addSegment(api.buildDelta([{ key: 0, texts: ["new"] }], [0]));
      let stale = "";
      try {
        index.explain(found.refs[0], "が😀");
      } catch (error) {
        stale = (error as { code: string }).code;
      }
      return { explained, stale };
    } finally {
      index.free();
    }
  });
  expect(result).toEqual({
    explained: {
      text: "ｶﾞ😀",
      level: 0,
      spans: [[0, 4]],
      coordinateSpace: "original-utf16",
    },
    stale: "INVALID_INPUT",
  });
});

test("intent DAG searches choices without expansion and explains original text", async ({
  page,
}) => {
  await page.goto("/tests/browser/fixtures/index.html");
  const result = await page.evaluate(async () => {
    // @ts-expect-error Browser-only absolute module URL.
    const api = await import("/packages/asearch-wasm/dist/web.mjs");
    const index = api.IntentIndex.fromEntries([
      { key: 1, expr: "(ｶ|ﾊ)ﾞ😀?" },
      { key: 2, expr: "(a|b)".repeat(50) },
    ]);
    try {
      const found = index.search("が😀", { maxAmbig: 0 });
      const explanation = index.explain(found.refs[0], "が😀", { maxAmbig: 0 });
      const many = index.search("a".repeat(50), { mode: 0, maxAmbig: 0 });
      return {
        ids: [...found.ids],
        engine: found.stats.engine,
        explanation,
        many: [...many.ids],
      };
    } finally {
      index.free();
    }
  });
  expect(result).toEqual({
    ids: [1],
    engine: "dag",
    explanation: {
      text: "ｶﾞ😀",
      level: 0,
      spans: [[0, 4]],
      coordinateSpace: "original-utf16",
    },
    many: [2],
  });
});
