import { cp, chmod, lstat, mkdir, mkdtemp, readFile, rename, rm, stat } from "node:fs/promises";
import { join, resolve, dirname, basename } from "node:path";
import { tmpdir } from "node:os";
import { findNative } from "./native";
import { buildProject } from "./build";

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
  const output = resolve(destination ?? join(source, "dist", manifest.slug + ".slop"));
  if (output === resolve(source) || !output.endsWith(".slop"))
    throw new Error("Template output must be a separate .slop directory");
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
    // Never replace a writable document with a build artifact.
    if (
      await stat(join(output, "state")).then(
        () => true,
        () => false,
      )
    )
      throw new Error("Refusing to overwrite a writable document");
    await mkdir(dirname(output), { recursive: true });
    const ready = output + ".ready-" + crypto.randomUUID();
    const previous = output + ".previous-" + crypto.randomUUID();
    await cp(stage, ready, { recursive: true });
    let moved = false;
    try {
      if (
        await stat(output).then(
          () => true,
          () => false,
        )
      ) {
        await rename(output, previous);
        moved = true;
      }
      await rename(ready, output);
    } catch (error) {
      if (moved) await rename(previous, output);
      throw error;
    } finally {
      await rm(ready, { recursive: true, force: true });
    }
    if (moved) await rm(previous, { recursive: true, force: true });
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
  if (
    await stat(join(destination, "state")).then(
      () => true,
      () => false,
    )
  )
    throw new Error("Refusing to replace a template containing writable document state");
  const ready = destination + ".ready-" + crypto.randomUUID();
  const backupRoot = join(dirname(dirname(destination)), "template-backups");
  const backup = join(backupRoot, basename(destination) + "." + crypto.randomUUID());
  await cp(source, ready, { recursive: true, errorOnExist: true, force: false });
  let moved = false;
  try {
    if (existing) {
      await mkdir(backupRoot, { recursive: true });
      await chmod(destination, existing.mode | 0o200);
      await rename(destination, backup);
      moved = true;
    }
    await rename(ready, destination);
  } catch (error) {
    if (moved) await rename(backup, destination);
    if (existing) await chmod(destination, existing.mode);
    throw error;
  } finally {
    await rm(ready, { recursive: true, force: true });
  }
}
