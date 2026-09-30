import { chmod, cp, lstat, mkdir, rename, rm, stat } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";

/** Whether `path` exists (as a regular file when `file`). Only a missing path is false. */
export async function exists(path: string, file = false): Promise<boolean> {
  try {
    const value = await stat(path);
    return !file || value.isFile();
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return false;
    throw error;
  }
}

/** A build's default output: `dist/<slug>.slop` inside the source. */
export const defaultOutput = (source: string, slug: string) => join(source, "dist", slug + ".slop");

/** Build outputs are separate `.slop` directories, and never a writable document. */
export async function assertReplaceable(output: string, source: string) {
  if (output === resolve(source) || !output.endsWith(".slop"))
    throw new Error("Build output must be a separate .slop directory");
  if (await exists(join(output, "state"))) throw new Error("Refusing to overwrite a writable document");
}

/**
 * Replaces `destination` with a copy of `source`. The copy is completed beside it, then
 * swapped in by rename, so readers see the old or the new directory, and a failed swap
 * restores the old one. The old directory moves to `backup` when given; otherwise it is
 * deleted.
 */
export async function replaceDirectory(source: string, destination: string, backup?: string) {
  const ready = destination + ".ready-" + crypto.randomUUID();
  const previous = backup ?? destination + ".previous-" + crypto.randomUUID();
  await mkdir(dirname(destination), { recursive: true });
  await cp(source, ready, { recursive: true, errorOnExist: true, force: false });
  const existing = await lstat(destination).catch((error) => {
    if (error.code === "ENOENT") return undefined;
    throw error;
  });
  let moved = false;
  try {
    if (existing) {
      await mkdir(dirname(previous), { recursive: true });
      // An immutable master directory cannot be renamed until it is writable.
      await chmod(destination, existing.mode | 0o200);
      await rename(destination, previous);
      moved = true;
    }
    await rename(ready, destination);
  } catch (error) {
    if (moved) await rename(previous, destination);
    if (existing) await chmod(destination, existing.mode);
    throw error;
  } finally {
    await rm(ready, { recursive: true, force: true });
  }
  if (moved && !backup) await rm(previous, { recursive: true, force: true });
}
