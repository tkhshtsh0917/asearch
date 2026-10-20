import { fork } from "node:child_process";
import { once } from "node:events";
import { fileURLToPath } from "node:url";

export async function startServer({ indexPath } = {}) {
  const child = fork(
    fileURLToPath(new URL("./server.mjs", import.meta.url)),
    [],
    {
      env: { ...process.env, ASEARCH_BENCH_INDEX: indexPath ?? "" },
      stdio: ["ignore", "ignore", "pipe", "ipc"],
    },
  );
  let errorOutput = "";
  child.stderr.on("data", (chunk) => {
    errorOutput += chunk;
  });
  try {
    const port = await new Promise((resolve, reject) => {
      const cleanup = () => {
        clearTimeout(timer);
        child.off("message", message);
        child.off("error", failed);
        child.off("exit", exited);
      };
      const failed = (error) => {
        cleanup();
        reject(error);
      };
      const exited = () =>
        failed(
          new Error(errorOutput || "Benchmark server exited before listening"),
        );
      const message = (value) => {
        if (
          !Number.isInteger(value?.port) ||
          value.port < 1 ||
          value.port > 65535
        )
          return failed(
            new Error("Invalid benchmark server readiness message"),
          );
        cleanup();
        resolve(value.port);
      };
      const timer = setTimeout(
        () => failed(new Error("Benchmark server readiness timed out")),
        10000,
      );
      child.on("message", message);
      child.once("error", failed);
      child.once("exit", exited);
    });
    return {
      url: `http://127.0.0.1:${port}/`,
      async close() {
        if (child.exitCode !== null || child.signalCode !== null) return;
        const exited = once(child, "exit");
        child.kill("SIGTERM");
        await exited;
      },
    };
  } catch (error) {
    child.kill("SIGTERM");
    throw error;
  }
}
