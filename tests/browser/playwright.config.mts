import { fileURLToPath } from "node:url";
import { defineConfig } from "@playwright/test";
export default defineConfig({
  testDir: ".",
  testMatch: "*.spec.ts",
  outputDir: "../../test-results/playwright",
  projects: (["chromium", "firefox", "webkit"] as const).map((name) => ({
    name,
    use: { browserName: name },
  })),
  use: {
    baseURL: "http://127.0.0.1:4387",
    headless: true,
  },
  webServer: {
    cwd: fileURLToPath(new URL("../../", import.meta.url)),
    command: "node tests/browser/server.mjs",
    url: "http://127.0.0.1:4387/tests/browser/fixtures/index.html",
    reuseExistingServer: false,
  },
});
