import { expect, test } from "@playwright/test";

const entry = "/packages/asearch-wasm/dist/web.mjs";
const wasmPath = "/packages/asearch-wasm/dist/web/asearch_wasm_bg.wasm";

for (const input of ["default", "url", "buffer", "module"]) {
  test(`failed initialization recovers with ${input} input`, async ({
    page,
  }) => {
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.route(`**${wasmPath}`, (route) =>
      route.fulfill({ status: 404, body: "missing" }),
    );
    await page.goto("/tests/browser/fixtures/index.html");
    const failure = await page.evaluate(async (entry) => {
      const module = await import(entry);
      try {
        await module.ready;
        return "unexpected success";
      } catch (error) {
        return (error as { code: string }).code;
      }
    }, entry);
    expect(failure).toBe("INITIALIZATION_FAILED");

    await page.unroute(`**${wasmPath}`);
    const result = await page.evaluate(
      async ({ entry, wasmPath, input }) => {
        const module = await import(entry);
        const url = new URL(wasmPath, location.href);
        let source: URL | ArrayBuffer | WebAssembly.Module | undefined;
        if (input === "url") source = url;
        else if (input !== "default") {
          const bytes = await (await fetch(url)).arrayBuffer();
          source =
            input === "module" ? await WebAssembly.compile(bytes) : bytes;
        }

        const attempt = module.init(source);
        await attempt;
        return {
          live: module.ready === attempt,
          stable: module.init() === attempt,
          matched: new module.Asearch("猫である", 1).match("吾輩は猫である。"),
        };
      },
      { entry, wasmPath, input },
    );
    expect(result).toEqual({ live: true, stable: true, matched: true });
    expect(errors).toEqual([]);
  });
}
