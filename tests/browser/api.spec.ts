import { expect, test } from "@playwright/test";

test("minimum levels and disposal preserve independent handles", async ({
  page,
}) => {
  await page.goto("/tests/browser/fixtures/index.html");
  const result = await page.evaluate(async () => {
    const { Asearch } = await import("/packages/asearch-wasm/dist/web.mjs");
    const a = new Asearch("a");
    const b = new Asearch("hello");
    const levels = [
      a.matchLevel("", 0),
      a.matchLevel(""),
      b.matchLevel("helo"),
    ];
    const own = Object.keys(a);
    a[Symbol.dispose ?? Symbol.for("Symbol.dispose")]();
    a.free();
    const failures: string[] = [];
    for (const call of [() => a.match("a"), () => a.matchLevel("a")]) {
      try {
        call();
      } catch (error) {
        failures.push((error as { code: string }).code);
      }
    }
    const other = b.match("helo", 1);
    b.free();
    return { levels, failures, other, own };
  });
  expect(result).toEqual({
    levels: [-1, 1, 1],
    failures: ["DISPOSED", "DISPOSED"],
    other: true,
    own: [
      "loopMask",
      "masks",
      "matchMode",
      "pattern",
      "initPattern",
      "acceptPattern",
    ],
  });
});

test("Ideal levels remain monotonic beyond the compatibility word width", async ({
  page,
}) => {
  await page.goto("/tests/browser/fixtures/index.html");
  const levels = await page.evaluate(async () => {
    const { Asearch } = await import("/packages/asearch-wasm/dist/web.mjs");
    const pattern = new Asearch("a", 0, { profile: "ideal" });
    const levels = [0, 1, 2, 31, 32].map((e) => pattern.match("", e));
    const minimum = pattern.matchLevel("");
    pattern.free();
    return { levels, minimum };
  });
  expect(levels).toEqual({
    levels: [false, true, true, true, true],
    minimum: 1,
  });
});
