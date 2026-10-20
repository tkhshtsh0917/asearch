import { readFile, writeFile } from "node:fs/promises";

// Reject a valid but unrelated WASM before wasm-bindgen caches its exports.
// Use the actual optimized artifact so export names cannot drift from the build.
export async function guardBindings(directory, target) {
  const module = new WebAssembly.Module(
    await readFile(`${directory}/asearch_wasm_bg.wasm`),
  );
  const expected = WebAssembly.Module.exports(module);
  const moduleName = target === "web" ? "module" : "wasmModule";
  const guard = `
    const actualExports = new Map(WebAssembly.Module.exports(${moduleName}).map(({name, kind}) => [name, kind]));
    if (!${JSON.stringify(expected)}.every(({name, kind}) => actualExports.get(name) === kind)) {
        throw new TypeError("Incompatible asearch WASM exports");
    }
`;

  const path = `${directory}/asearch_wasm.js`;
  const source = await readFile(path, "utf8");
  const marker =
    target === "web"
      ? "function __wbg_finalize_init(instance, module) {"
      : "let wasm = wasmInstance.exports;";
  if (source.split(marker).length !== 2) {
    throw new Error(
      "wasm-bindgen output changed; review the export validation hook",
    );
  }
  await writeFile(
    path,
    source.replace(marker, target === "web" ? marker + guard : guard + marker),
  );
}
