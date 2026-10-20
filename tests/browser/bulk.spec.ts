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
