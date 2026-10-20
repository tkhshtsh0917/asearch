import { expect, test } from "@playwright/test";

const entry = "/packages/asearch-wasm/dist/web.mjs";
const wasm = "**/packages/asearch-wasm/dist/web/asearch_wasm_bg.wasm";

test("browser fixture subset, README and module Worker need no process", async ({
  page,
}) => {
  await page.goto("/tests/browser/fixtures/index.html");
  const result = await page.evaluate(async (entry) => {
    const { Asearch } = await import(entry);
    const fixture = await (
      await fetch("/crates/asearch-rs/tests/fixtures/compat.json")
    ).json();
    const selected = fixture.cases
      .filter((c: { ctor: { ok: unknown } }) => c.ctor.ok)
      .slice(0, 12);
    for (const p of ["😀", "a😀b", "a  b", "漢字文字列"])
      selected.push(
        fixture.cases.find(
          (c: { pattern: number[]; mode: { json: unknown } }) =>
            String.fromCharCode(...c.pattern) === p && c.mode.json === 2,
        ),
      );
    for (const c of fixture.cases
      .filter((c: { ctor: { ok: unknown } }) => c.ctor.ok)
      .slice(12, 16))
      selected.push(c);
    let checks = 0;
    for (const c of selected)
      for (const [profile, key] of [
        ["asearch.ts", "src"],
        ["asearch.ts-lib", "lib"],
      ]) {
        const a = new Asearch(
          String.fromCharCode(...c.pattern),
          c.mode.nan ? NaN : c.mode.json,
          { profile },
        );
        for (let ti = 0; ti < c.texts.length; ti++)
          for (let ei = 0; ei < fixture.ambigs.length; ei++) {
            if (
              a.match(
                String.fromCharCode(...c.texts[ti]),
                fixture.ambigs[ei],
              ) !== c[key][ti][ei]
            )
              throw new Error(c.id);
            checks++;
          }
      }
    const worker = await new Promise((resolve, reject) => {
      const w = new Worker("/tests/browser/fixtures/worker.mjs", {
        type: "module",
      });
      w.onmessage = (e) => {
        resolve(e.data);
        w.terminate();
      };
      w.onerror = (e) => {
        w.terminate();
        reject(new Error(e.message));
      };
    });
    return {
      checks,
      cases: selected.length,
      worker,
      process: typeof process,
      example: new Asearch("hello").match("helo", 1),
    };
  }, entry);
  expect(result.cases).toBe(20);
  expect(result.checks).toBeGreaterThan(0);
  expect(result.process).toBe("undefined");
  expect(result.example).toBe(true);
  expect(result.worker).toEqual({
    matched: true,
    process: "undefined",
    session: [0, 2],
    optimized: true,
  });
});

for (const kind of ["missing", "corrupt", "incompatible"]) {
  test(`initialization ${kind}, retry and Promise lifecycle`, async ({
    page,
  }) => {
    const errors: string[] = [];
    page.on("pageerror", (e) => errors.push(e.message));
    await page.route(wasm, (route) =>
      route.fulfill(
        kind === "missing"
          ? { status: 404, body: "missing" }
          : {
              status: 200,
              contentType: "application/wasm",
              body:
                kind === "incompatible"
                  ? Buffer.from([0, 97, 115, 109, 1, 0, 0, 0])
                  : "broken",
            },
      ),
    );
    await page.goto("/tests/browser/fixtures/index.html");
    const failure = await page.evaluate(async (entry) => {
      const m = await import(entry);
      const first = m.ready;
      let code: string | undefined;
      try {
        await first;
      } catch (e) {
        code = (e as { code: string }).code;
      }
      let ctor = false;
      try {
        new m.Asearch("a");
      } catch {
        ctor = true;
      }
      (globalThis as unknown as { module: unknown; first: unknown }).module = m;
      (globalThis as unknown as { first: unknown }).first = first;
      return { code, ctor };
    }, entry);
    expect(failure).toEqual({ code: "INITIALIZATION_FAILED", ctor: true });
    await page.unroute(wasm);
    const recovered = await page.evaluate(async () => {
      const g = globalThis as unknown as {
        module: {
          init: (i?: Uint8Array) => Promise<void>;
          ready: Promise<void>;
          Asearch: new (p: string) => { match: (t: string) => boolean };
        };
        first: Promise<void>;
      };
      const bytes = new Uint8Array(
        await (
          await fetch("/packages/asearch-wasm/dist/web/asearch_wasm_bg.wasm")
        ).arrayBuffer(),
      );
      const retry = g.module.init(bytes);
      const same = g.module.init() === retry;
      let busy: string | undefined;
      try {
        await g.module.init(bytes);
      } catch (e) {
        busy = (e as { code: string }).code;
      }
      await retry;
      let old: string | undefined;
      try {
        await g.first;
      } catch (e) {
        old = (e as { code: string }).code;
      }
      let after: string | undefined;
      try {
        await g.module.init(bytes);
      } catch (e) {
        after = (e as { code: string }).code;
      }
      return {
        same,
        busy,
        old,
        after,
        ready: g.module.ready === retry,
        match: new g.module.Asearch("a").match("A"),
      };
    });
    expect(recovered).toEqual({
      same: true,
      busy: "INITIALIZATION_IN_PROGRESS",
      old: "INITIALIZATION_FAILED",
      after: "ALREADY_INITIALIZED",
      ready: true,
      match: true,
    });
    expect(errors).toEqual([]);
  });
}

test("tarball web entry works without the repository dist", async ({
  page,
}) => {
  await page.route("**/packages/asearch-wasm/dist/**", (route) =>
    route.abort(),
  );
  await page.goto("/tests/browser/fixtures/index.html");
  const result = await page.evaluate(async () => {
    const { Asearch, MatchMode, ready } = await import("/packed/dist/web.mjs");
    await ready;
    return new Asearch("猫である", MatchMode.Include).match("吾輩は猫である。");
  });
  expect(result).toBe(true);
});
