import { build } from "vite";
import { cp, mkdtemp, rm, stat } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { publishFolder, repository, shellDestinations } from "../lib/artifacts";

/** Builds the page shell once and installs it in the app and CLI. */
export async function buildShell() {
  const stage = await mkdtemp(join(tmpdir(), "hitslop-shell-"));
  try {
    await cp(join(repository, "packages/hitslop/src/shell/boot.js"), join(stage, "boot.js"));
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
          input: join(repository, "packages/hitslop/src/shell/boot.ts"),
          preserveEntrySignatures: "strict",
          output: { format: "es", entryFileNames: "index.js" },
        },
      },
    });
    const { size } = await stat(join(stage, "index.js"));
    console.log(`Page shell: ${(size / 1024).toFixed(1)} KB`);
    for (const destination of Object.values(shellDestinations))
      await publishFolder(destination, async (ready) => {
        await cp(stage, ready, { recursive: true });
      });
  } finally {
    await rm(stage, { recursive: true, force: true });
  }
}
if (import.meta.main) await buildShell();
