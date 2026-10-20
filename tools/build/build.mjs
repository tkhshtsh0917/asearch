import { spawnSync } from "node:child_process";
import { copyFile, cp, mkdir, rm, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "esbuild";
import { guardBindings } from "./guard-bindings.mjs";

const root = fileURLToPath(new URL("../../", import.meta.url));
// All tools and filesystem operations must use this repository, not the caller's cwd.
process.chdir(root);
function run(command, args, env = {}) {
  const result = spawnSync(command, args, {
    cwd: root,
    stdio: "inherit",
    env: { ...process.env, ...env },
  });
  if (result.status !== 0) throw new Error(`${command} failed`);
}
const sysroot = spawnSync("rustc", ["--print", "sysroot"], {
  encoding: "utf8",
});
if (sysroot.status !== 0) throw new Error("Cannot resolve Rust sysroot");
const rustFlags = [
  `--remap-path-prefix=${homedir()}=/build/home`,
  `--remap-path-prefix=${process.env.CARGO_HOME ?? `${homedir()}/.cargo`}=/build/cargo`,
  `--remap-path-prefix=${root}=/workspace/`,
  `--remap-path-prefix=${sysroot.stdout.trim()}=/build/rust`,
];
const metadata = spawnSync(
  "cargo",
  ["metadata", "--locked", "--no-deps", "--format-version", "1"],
  {
    encoding: "utf8",
  },
);
if (metadata.status !== 0)
  throw new Error("Cannot resolve Cargo target directory");
const crate = resolve(
  JSON.parse(metadata.stdout).target_directory,
  "wasm32-unknown-unknown/release/asearch_wasm.wasm",
);
const dist = "packages/asearch-wasm/dist";
run(
  "cargo",
  [
    "build",
    "--locked",
    "-p",
    "asearch-wasm",
    "--target",
    "wasm32-unknown-unknown",
    "--release",
  ],
  { CARGO_ENCODED_RUSTFLAGS: rustFlags.join("\x1f") },
);
// Never ship files left behind by a previous build.
await rm(dist, { recursive: true, force: true });

for (const target of ["web", "nodejs"]) {
  const directory = target === "web" ? "web" : "node";
  run("wasm-bindgen", [
    "--target",
    target,
    "--out-dir",
    `${dist}/${directory}`,
    crate,
  ]);
  run("pnpm", [
    "exec",
    "wasm-opt",
    "-O3",
    "--enable-bulk-memory",
    "--enable-nontrapping-float-to-int",
    "--enable-sign-ext",
    "--enable-mutable-globals",
    "--enable-reference-types",
    "--enable-multivalue",
    "-o",
    `${dist}/${directory}/asearch_wasm_bg.wasm`,
    `${dist}/${directory}/asearch_wasm_bg.wasm`,
  ]);
  await guardBindings(`${dist}/${directory}`, target);
}
await writeFile(`${dist}/node/package.json`, '{"type":"commonjs"}\n');
for (const [entry, format, name] of [
  ["web", "esm", "web.mjs"],
  ["node", "esm", "node.mjs"],
  ["node", "cjs", "node.cjs"],
]) {
  await build({
    absWorkingDir: root,
    entryPoints: [`packages/asearch-wasm/src/${entry}.ts`],
    outfile: `${dist}/${name}`,
    bundle: true,
    format,
    platform: entry === "node" ? "node" : "browser",
    target: "es2022",
    plugins: [
      {
        name: "bindings",
        setup(plugin) {
          plugin.onResolve(
            { filter: /dist\/(web|node)\/asearch_wasm\.js$/ },
            ({ path }) => ({
              path: path.replace("../dist/", "./"),
              external: true,
            }),
          );
        },
      },
    ],
  });
}
run("pnpm", ["exec", "tsc", "-p", "packages/asearch-wasm/tsconfig.json"]);
await copyFile(`${dist}/index.d.ts`, `${dist}/index.d.cts`);
for (const name of ["LICENSE-MIT", "LICENSE-APACHE", "NOTICE"])
  await cp(name, `packages/asearch-wasm/${name}`);
await mkdir(`${dist}/licenses`, { recursive: true });
await cp("vendor/upstream/licenses", `${dist}/licenses`, { recursive: true });
await copyFile(
  "vendor/unicode/LICENSE.txt",
  `${dist}/licenses/Unicode-3.0.txt`,
);
