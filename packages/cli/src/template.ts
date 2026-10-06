import { copyFile, lstat, mkdir, mkdtemp, readdir, readFile, rm } from "node:fs/promises";
import { join, resolve, dirname, basename } from "node:path";
import { tmpdir } from "node:os";
import { findNative, negotiate } from "./native";
import { buildProject } from "./build";
import { checkTemplatePackage } from "./package-check";
import { assertReplaceable, defaultOutput, exists, replaceDirectory } from "./fs";

async function run(command: string[]) {
  const child = Bun.spawn(command, { stdout: "pipe", stderr: "pipe" });
  const [output, error, code] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  if (code) throw new Error(`${command[0]} failed: ${error || output}`);
}

/** Native artwork comes from the installed app's helper, never a compiler or checkout,
 * through the same protocol check as document commands. Returns the helper's command. */
export async function prepareRenderer() {
  if (process.platform !== "darwin")
    throw new Error("--artwork native renders with hitSlop.app on macOS. Elsewhere, add artwork/preview.png and artwork/icon.png to the project.");
  return negotiate(await findNative());
}

/** Artwork the author supplies, by the name it takes in `QuickLook/`. */
const suppliedArtwork = [
  ["artwork/preview.png", "Preview.png"],
  ["artwork/icon.png", "Icon.png"],
] as const;

/** Publish only a completed, checked artifact. Supplied artwork is copied; with a
 * `render` command (from `prepareRenderer`), the app renders what was not supplied,
 * reading the stage into disposable memory storage. Without one, the build uses no
 * helper and needs no Mac. */
export async function buildTemplate(source: string, render: string[] | undefined, destination?: string) {
  const manifest = JSON.parse(await readFile(join(source, "manifest.json"), "utf8"));
  const output = resolve(destination ?? defaultOutput(source, manifest.slug));
  await assertReplaceable(output, source);
  const temporary = await mkdtemp(join(tmpdir(), "hitslop-template-"));
  const stage = join(temporary, "Template.slop");
  try {
    await buildProject(source, stage);
    await mkdir(join(stage, "QuickLook"));
    for (const [from, name] of suppliedArtwork)
      if (await exists(join(source, from), true)) await copyFile(join(source, from), join(stage, "QuickLook", name));
    const quickLook = (name: string) => exists(join(stage, "QuickLook", name), true);
    if (render && !(await quickLook("Preview.png")))
      await run([...render, "screenshot", stage, "--target", "preview", "--output", join(stage, "QuickLook/Preview.png")]);
    if (render && !(await quickLook("Icon.png"))) {
      await run([...render, "screenshot", stage, "--target", "icon", "--if-present", "--output", join(stage, "QuickLook/Icon.png")]);
      if (!(await quickLook("Icon.png"))) console.warn("No icon view; Finder will show the generic icon.");
    }
    if (!render && !(await quickLook("Preview.png")))
      console.warn("No artwork: add artwork/preview.png and artwork/icon.png, or build with --artwork native on a Mac.");
    if (!(await readdir(join(stage, "QuickLook"))).length) await rm(join(stage, "QuickLook"), { recursive: true });
    await checkTemplatePackage(stage);
    // The output may have become a document while rendering.
    await assertReplaceable(output, source);
    await replaceDirectory(stage, output);
    return output;
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
}

export async function installTemplate(source: string, destination: string) {
  const existing = await lstat(destination).catch((error) => {
    if (error.code === "ENOENT") return undefined;
    throw error;
  });
  if (existing?.isSymbolicLink() || (existing && !existing.isDirectory()))
    throw new Error("Registered template must be a directory, not a link");
  if (await exists(join(destination, "state")))
    throw new Error("Refusing to replace a template containing writable document state");
  const backup = join(dirname(dirname(destination)), "template-backups", basename(destination) + "." + crypto.randomUUID());
  await replaceDirectory(source, destination, backup);
}
