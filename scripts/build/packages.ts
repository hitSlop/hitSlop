import { copyFile, mkdir, readFile, readdir, rm } from "node:fs/promises";
import { resolve, join } from "node:path";
import { stageEngines } from "./engines";
import { buildSkills } from "../../packages/tinys/src/cli/skills-build";
import { buildEngine, cargoOutput } from "./core";
import { buildBrowser } from "./browser";
import { buildShell } from "./shell";
/** The npm package (`generated/npm`), carrying the file engines it ships. */
export async function packPackages(prepared = false) {
  if (!prepared) {
    await buildShell();
    await buildBrowser();
    await buildEngine();
  }
  await buildSkills();
  const output = resolve("generated/npm");
  await mkdir(output, { recursive: true });

  /** Removed after packing, so a checkout always runs its fresh build. */
  const engines = resolve("packages/tinys/engine");
  try {
    const commit = (await new Response(Bun.spawn(["git", "rev-parse", "HEAD"], { stdout: "pipe" }).stdout).text()).trim();
    await stageEngines(engines, resolve("generated/engines"), cargoOutput("tiny-engine"), commit);
    const notices = join(engines, "font-licenses");
    await mkdir(notices);
    const fonts = resolve("crates/tinys-runner/fonts");
    for (const name of await readdir(fonts))
      if (name.includes("LICENSE") || name === "README.md") await copyFile(join(fonts, name), join(notices, name));
    const rendererNotices = join(engines, "renderer-licenses");
    await mkdir(rendererNotices);
    const rendererLicenseSource = resolve("crates/tinys-runner/renderer-licenses");
    for (const name of await readdir(rendererLicenseSource))
      await copyFile(join(rendererLicenseSource, name), join(rendererNotices, name));
    const directory = resolve("packages/tinys");
    const metadata = JSON.parse(await readFile(join(directory, "package.json"), "utf8"));
    for (const value of Object.values(metadata.dependencies ?? {}))
      if (String(value).startsWith("workspace:") || String(value).startsWith("file:"))
        throw new Error("Local dependency in published package");
    const child = Bun.spawn([process.execPath, "pm", "pack", "--destination", output], {
      cwd: directory,
      stdout: "inherit",
      stderr: "inherit",
    });
    if (await child.exited) throw new Error("Cannot pack tinys");
  } finally {
    await rm(engines, { recursive: true, force: true });
  }
}

if (import.meta.main) await packPackages();
