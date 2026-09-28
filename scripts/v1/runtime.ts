import { build as esbuild } from "esbuild";
import { mkdir, cp, mkdtemp, rm, rename } from "node:fs/promises";
import { join, dirname } from "node:path";
import { tmpdir } from "node:os";
import { repository, shellDestinations } from "./runtime-artifacts";

/** Builds the page shell once and installs it in the app and CLI (plus WASM for CLI dev). */
export async function buildShell() {
  const stage = await mkdtemp(join(tmpdir(), "hitslop-shell-"));
  try {
    await cp(join(repository, "packages/document/src/boot.js"), join(stage, "boot.js"));
    await esbuild({
      entryPoints: [join(repository, "packages/document/src/runtime-entry.ts")],
      outfile: join(stage, "index.js"),
      bundle: true,
      format: "esm",
      platform: "browser",
      target: "safari17",
      minify: true,
    });
    for (const [consumer, destination] of Object.entries(shellDestinations)) {
      await mkdir(dirname(destination), { recursive: true });
      const ready = destination + ".building";
      await rm(ready, { recursive: true, force: true });
      await cp(stage, ready, { recursive: true });
      if (consumer === "cli")
        await cp(join(repository, "generated/v1/core/wasm"), join(ready, "core"), { recursive: true });
      await rm(destination, { recursive: true, force: true });
      await rename(ready, destination);
    }
  } finally {
    await rm(stage, { recursive: true, force: true });
  }
}
if (import.meta.main) await buildShell();
