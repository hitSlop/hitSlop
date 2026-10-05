import { Database } from "bun:sqlite";
import { createHash } from "node:crypto";
import { lstat, readdir, readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { brotliDecompressSync } from "node:zlib";

export const repository = resolve(import.meta.dir, "..");
/** Test runs keep their own writer-lock registry, so they never fill a person's
 * `~/.hitslop/live`. Debug hosts and the file engine honor it; release builds never do. */
export function useTestRegistry() {
  process.env.HITSLOP_TEST_REGISTRY ||= join(tmpdir(), "hitslop-test-registry");
}
/** The page shell the host injects: the app bundles it; the CLI adds the WASM core for dev. */
export const shellDestinations = {
  app: join(repository, "apps/apple/Packages/HitSlopApple/Sources/HitSlopDocument/Resources/shell"),
  cli: join(repository, "packages/cli/shell"),
};
export const shellFiles = ["boot.js", "index.js"] as const;
/** An app asset's text as the build wrote it, read from the file outside the core, which
 * stores text Brotli-compressed. */
export function appAsset(file: string, path: string): string {
  const database = new Database(file, { readonly: true });
  try {
    const row = database.query("SELECT encoding, bytes FROM assets WHERE path = ?").get(path) as { encoding: string; bytes: Uint8Array };
    return (row.encoding === "br" ? brotliDecompressSync(row.bytes) : Buffer.from(row.bytes)).toString("utf8");
  } finally {
    database.close();
  }
}
/** SHA-256 in hex. */
export const sha256 = (data: string | Uint8Array) => createHash("sha256").update(data).digest("hex");
/** A regular file's SHA-256 in hex. */
export async function fileDigest(path: string): Promise<string> {
  if (!(await lstat(path)).isFile()) throw new Error(`Not a file: ${path}`);
  return sha256(await readFile(path));
}
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
