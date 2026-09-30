import { createHash } from "node:crypto";
import { lstat, readdir, readFile } from "node:fs/promises";
import { join, resolve } from "node:path";

export const repository = resolve(import.meta.dir, "..");
/** The page shell the host injects: the app bundles it; the CLI adds the WASM core for dev. */
export const shellDestinations = {
  app: join(repository, "apps/apple/Packages/HitSlopApple/Sources/HitSlopDocument/Resources/shell"),
  cli: join(repository, "packages/cli/shell"),
};
export const shellFiles = ["boot.js", "index.js"] as const;
export async function digest(root: string, topLevel?: readonly string[]): Promise<string> {
  const hash = createHash("sha256");
  async function visit(prefix: string) {
    const names = prefix === "" && topLevel ? topLevel : await readdir(join(root, prefix));
    for (const name of [...names].sort()) {
      const relative = join(prefix, name),
        path = join(root, relative),
        info = await lstat(path);
      if (info.isSymbolicLink() || (!info.isDirectory() && !info.isFile()))
        throw new Error(`Invalid package resource: ${path}`);
      hash.update(JSON.stringify([relative, info.isDirectory() ? "directory" : "file"]));
      if (info.isDirectory()) await visit(relative);
      else hash.update(createHash("sha256").update(await readFile(path)).digest());
    }
  }
  await visit("");
  return hash.digest("hex");
}

/** The app and the CLI must serve byte-identical page shells. */
export async function verifyShellCopies(roots: string[] = Object.values(shellDestinations)) {
  const values = await Promise.all(roots.map((root) => digest(root, shellFiles)));
  if (values.some((value) => value !== values[0]))
    throw new Error("Page shell bytes differ between consumers; run bun run build");
  return values[0]!;
}
