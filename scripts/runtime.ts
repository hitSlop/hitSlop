import { build } from "vite";
import { mkdir, cp, mkdtemp, rm, rename, stat } from "node:fs/promises";
import { join, dirname } from "node:path";
import { tmpdir } from "node:os";
import { repository, shellDestinations } from "./runtime-artifacts";

/** Every window parses the shell before the app, so a heavy dependency (such as a schema
 * validator; the core validates requests) fails the build. */
const shellBudget = 64 * 1024;

/** Builds the page shell once and installs it in the app and CLI (plus WASM for CLI dev). */
export async function buildShell() {
  const stage = await mkdtemp(join(tmpdir(), "hitslop-shell-"));
  try {
    await cp(join(repository, "packages/shell/src/boot.js"), join(stage, "boot.js"));
    await build({
      configFile: false,
      logLevel: "warn",
      build: {
        outDir: stage,
        emptyOutDir: false,
        target: "safari17",
        minify: true,
        copyPublicDir: false,
        modulePreload: false,
        // One module exporting `boot`, which boot.js imports.
        rolldownOptions: {
          input: join(repository, "packages/shell/src/boot.ts"),
          preserveEntrySignatures: "strict",
          output: { format: "es", entryFileNames: "index.js" },
        },
      },
    });
    const { size } = await stat(join(stage, "index.js"));
    if (size > shellBudget)
      throw new Error(`Page shell is ${size} bytes, over its ${shellBudget}-byte budget`);
    for (const [consumer, destination] of Object.entries(shellDestinations)) {
      await mkdir(dirname(destination), { recursive: true });
      const ready = destination + ".building";
      await rm(ready, { recursive: true, force: true });
      await cp(stage, ready, { recursive: true });
      if (consumer === "cli")
        await cp(join(repository, "generated/core/wasm"), join(ready, "core"), { recursive: true });
      await rm(destination, { recursive: true, force: true });
      await rename(ready, destination);
    }
  } finally {
    await rm(stage, { recursive: true, force: true });
  }
}
if (import.meta.main) await buildShell();
