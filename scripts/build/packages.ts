import { mkdir, readFile, rm } from "node:fs/promises";
import { resolve, join } from "node:path";
import { stageEngines } from "./engines";
import { buildSkills } from "../../packages/hitslop/src/cli/skills-build";
import { buildEngine, cargoOutput } from "./core";
import { buildShell } from "./shell";
/** The npm package (`generated/npm`), carrying the file engines it ships. */
export async function packPackages() {
  await buildShell();
  await buildSkills();
  await buildEngine();
  const output = resolve("generated/npm");
  await mkdir(output, { recursive: true });

  /** Removed after packing, so a checkout always runs its fresh build. */
  const engines = resolve("packages/hitslop/engine");
  try {
    const commit = (await new Response(Bun.spawn(["git", "rev-parse", "HEAD"], { stdout: "pipe" }).stdout).text()).trim();
    await stageEngines(engines, resolve("generated/engines"), cargoOutput("slop-engine"), commit);
    const directory = resolve("packages/hitslop");
    const metadata = JSON.parse(await readFile(join(directory, "package.json"), "utf8"));
    for (const value of Object.values(metadata.dependencies ?? {}))
      if (String(value).startsWith("workspace:") || String(value).startsWith("file:"))
        throw new Error("Local dependency in published package");
    const child = Bun.spawn([process.execPath, "pm", "pack", "--destination", output], {
      cwd: directory,
      stdout: "inherit",
      stderr: "inherit",
    });
    if (await child.exited) throw new Error("Cannot pack hitslop");
  } finally {
    await rm(engines, { recursive: true, force: true });
  }
}

if (import.meta.main) await packPackages();
