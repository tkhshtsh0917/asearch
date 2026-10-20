import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../../", import.meta.url));
function run(command, args, cwd) {
  const p = spawnSync(command, args, {
    cwd,
    encoding: "utf8",
    env: {
      ...process.env,
      npm_config_cache: join(tmpdir(), "asearch-m0-npm-cache"),
    },
  });
  assert.equal(p.status, 0, p.stderr || p.stdout);
  return p.stdout;
}
function checkConsumerTypes(directory) {
  const source = `import { Asearch, MatchMode, init, ready, type Options } from "asearch-wasm";
const options: Options = { profile: "asearch.ts-lib" };
const pattern = new Asearch("hello", MatchMode.Include, options);
const result: boolean = pattern.match("helo", 1);
const pending: Promise<void> = init(new Uint8Array());
const level: number = pattern.matchLevel("helo");
pattern.free();
pattern[Symbol.dispose]();
const initialized: Promise<void> = ready;
// @ts-expect-error A missing or any-typed declaration must not silently pass.
new Asearch(123);
void [result, pending, initialized, level];
`;
  for (const name of ["consumer.mts", "consumer.cts", "worker.mts"])
    writeFileSync(join(directory, name), source);

  for (const worker of [false, true]) {
    writeFileSync(
      join(directory, "tsconfig.json"),
      JSON.stringify({
        compilerOptions: {
          target: "ES2022",
          module: worker ? "ESNext" : "NodeNext",
          moduleResolution: worker ? "Bundler" : "NodeNext",
          lib: ["ES2022", "ESNext.Disposable", worker ? "WebWorker" : "DOM"],
          strict: true,
          noEmit: true,
        },
        files: worker ? ["worker.mts"] : ["consumer.mts", "consumer.cts"],
      }),
    );
    run(
      process.execPath,
      [join(root, "node_modules/typescript/bin/tsc"), "-p", "tsconfig.json"],
      directory,
    );
  }
}

test("packed distribution works alone, with both Node export conditions", () => {
  const temp = mkdtempSync(join(tmpdir(), "asearch-pack-"));
  try {
    const packed = JSON.parse(
      run(
        "npm",
        [
          "pack",
          "--workspace",
          "packages/asearch-wasm",
          "--json",
          "--pack-destination",
          temp,
        ],
        root,
      ),
    )[0];
    for (const file of packed.files) {
      assert.ok(
        !/^(?:src|test|tests|smoke|examples)\//.test(file.path),
        file.path,
      );
      assert.ok(!file.path.endsWith(".rs"), file.path);
    }
    run("tar", ["-xzf", join(temp, packed.filename), "-C", temp], temp);
    mkdirSync(join(temp, "node_modules"));
    renameSync(
      join(temp, "package"),
      join(temp, "node_modules", "asearch-wasm"),
    );
    for (const args of [
      [
        "--input-type=module",
        "-e",
        "import { Asearch } from 'asearch-wasm'; console.log(new Asearch('a').match('A'));",
      ],
      [
        "-e",
        "const {Asearch}=require('asearch-wasm');console.log(new Asearch('a').match('A'));",
      ],
    ])
      assert.equal(run(process.execPath, args, temp).trim(), "true");
    const pkg = join(temp, "node_modules", "asearch-wasm");
    assert.match(readFileSync(join(pkg, "NOTICE"), "utf8"), /asearch.ts 1.0.3/);
    const unicodeLicense = readFileSync(
      join(root, "vendor", "unicode", "LICENSE.txt"),
      "utf8",
    );
    assert.equal(
      readFileSync(join(pkg, "dist", "licenses", "Unicode-3.0.txt"), "utf8"),
      unicodeLicense,
    );
    assert.equal(
      readFileSync(
        join(root, "crates", "asearch-rs", "LICENSE-Unicode-3.0.txt"),
        "utf8",
      ),
      unicodeLicense,
    );
    checkConsumerTypes(temp);
    for (const entry of ["node.mjs", "node.cjs"]) {
      const wasm = join(pkg, "dist", "node", "asearch_wasm_bg.wasm");
      const bytes = readFileSync(wasm);
      for (const fault of ["missing", "corrupt", "incompatible"]) {
        if (fault === "missing") rmSync(wasm);
        else
          writeFileSync(
            wasm,
            fault === "incompatible"
              ? new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0])
              : "broken",
          );

        const p = spawnSync(
          process.execPath,
          [
            "-e",
            entry.endsWith("mjs")
              ? `import(${JSON.stringify(join(pkg, "dist", entry))})`
              : `require(${JSON.stringify(join(pkg, "dist", entry))})`,
          ],
          { encoding: "utf8" },
        );

        assert.notEqual(p.status, 0);
        assert.match(
          p.stderr,
          fault === "missing"
            ? /ENOENT/
            : fault === "incompatible"
              ? /Incompatible asearch WASM exports/
              : /CompileError/,
        );
        writeFileSync(wasm, bytes);
      }
    }
  } finally {
    rmSync(temp, { recursive: true, force: true });
  }
});
