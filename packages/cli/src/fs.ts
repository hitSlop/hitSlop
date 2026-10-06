import { stat } from "node:fs/promises";
import { join } from "node:path";

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
