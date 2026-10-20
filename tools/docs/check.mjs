import assert from "node:assert/strict";
import { readdir, readFile, stat } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));
async function documents(directory) {
  const paths = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) paths.push(...(await documents(path)));
    else {
      assert.ok(entry.name.endsWith(".md"), `Non-document in docs: ${path}`);
      paths.push(path);
    }
  }
  return paths;
}

const files = [
  ...(await documents(resolve(root, "docs"))),
  ...[
    "README.md",
    "AGENTS.md",
    "tools/reference/README.md",
    "tools/bench/README.md",
    "tests/browser/README.md",
    "vendor/README.md",
    "vendor/unicode/README.md",
    "packages/asearch-wasm/README.md",
  ].map((path) => resolve(root, path)),
];
let links = 0;
for (const file of files) {
  const text = (await readFile(file, "utf8")).replace(
    /```[^\n]*\n[\s\S]*?```/g,
    "",
  );
  for (const [, raw] of text.matchAll(/\]\(([^)]+)\)/g)) {
    if (/^(?:[a-z]+:|\/\/)/i.test(raw)) continue;
    const [path, fragment] = raw.split("#");
    const target = path
      ? resolve(dirname(file), decodeURIComponent(path))
      : file;
    assert.ok((await stat(target)).isFile(), `Broken link: ${file} → ${raw}`);
    if (fragment) {
      const body = await readFile(target, "utf8");
      const id = decodeURIComponent(fragment);
      const slugs = [...body.matchAll(/^#{1,6}\s+(.+)$/gm)].map(([, title]) =>
        title
          .toLowerCase()
          .replace(/[^\p{L}\p{N}\p{M}_\-\s]/gu, "")
          .replace(/ /g, "-"),
      );
      assert.ok(
        body.includes(`id="${id}"`) || slugs.includes(id),
        `Broken fragment: ${file} → ${raw}`,
      );
    }
    links++;
  }
}
console.log(`${files.length} documents, ${links} local links checked`);
