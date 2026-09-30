import { lstat, mkdir, mkdtemp, readFile, rm } from "node:fs/promises";
import { join, resolve, dirname, basename } from "node:path";
import { tmpdir } from "node:os";
import { findNative } from "./native";
import { buildProject } from "./build";
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

/** User builds use the installed app, never a compiler or checkout. */
export async function prepareRenderer() {
  if (process.platform !== "darwin")
    throw new Error("Native template artwork requires hitSlop.app on macOS.");
  const helper = await findNative();
  return helper;
}

/** Publish only a completed artifact. Native rendering reads the stage into disposable memory storage. */
export async function buildTemplate(source: string, renderer: string, destination?: string) {
  const manifest = JSON.parse(await readFile(join(source, "manifest.json"), "utf8"));
  const output = resolve(destination ?? defaultOutput(source, manifest.slug));
  await assertReplaceable(output, source);
  const temporary = await mkdtemp(join(tmpdir(), "hitslop-template-"));
  const stage = join(temporary, "Template.slop");
  try {
    await buildProject(source, stage);
    await mkdir(join(stage, "QuickLook"));
    await run([
      renderer,
      "screenshot",
      stage,
      "--target",
      "preview",
      "--output",
      join(stage, "QuickLook/Preview.png"),
    ]);
    await run([
      renderer,
      "screenshot",
      stage,
      "--target",
      "icon",
      "--if-present",
      "--output",
      join(stage, "QuickLook/Icon.png"),
    ]);
    if (!(await exists(join(stage, "QuickLook/Icon.png"))))
      console.warn("No icon view; Finder will show the generic icon.");
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
