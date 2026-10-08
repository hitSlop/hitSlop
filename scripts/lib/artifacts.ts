import { Database } from "bun:sqlite";
import { createHash } from "node:crypto";
import { lstat, mkdir, readdir, readFile, rename, rm, writeFile } from "node:fs/promises";
import { mkdtempSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { brotliDecompressSync } from "node:zlib";

export const repository = resolve(import.meta.dir, "../..");
/** Test runs keep their own writer-lock registry, so they never fill a person's
 * `~/.hitslop/live`. Debug hosts and the file engine honor it; release builds never do. */
export function useTestRegistry() {
  process.env.HITSLOP_EVALUATOR ||= join(repository, "target", process.env.HITSLOP_CARGO_PROFILE || "release", "hitslop-evaluator");
  if (!process.env.HITSLOP_TEST_REGISTRY) {
    const registry = mkdtempSync(join(tmpdir(), "hitslop-test-registry-"));
    process.env.HITSLOP_TEST_REGISTRY = registry;
    process.once("exit", () => rmSync(registry, { recursive: true, force: true }));
  }
}
/** The page shell the host injects: the app bundles it; the CLI serves the same shell over its native preview bridge. */
export const shellDestinations = {
  app: join(repository, "apps/apple/Packages/HitSlopApple/Sources/HitSlopDocument/Resources/shell"),
  cli: join(repository, "packages/hitslop/shell"),
};
export const shellFiles = ["boot.js", "index.js"] as const;
/** An app asset's text as the build wrote it, read from the file outside the core, which
 * stores text Brotli-compressed. */
export function appAsset(file: string, path: string): string {
  const database = new Database(file, { readonly: true });
  try {
    const row = database.query("SELECT encoding, bytes FROM assets WHERE key = ?").get(path) as { encoding: string; bytes: Uint8Array };
    return (row.encoding === "br" ? brotliDecompressSync(row.bytes) : Buffer.from(row.bytes)).toString("utf8");
  } finally {
    database.close();
  }
}
/** Fills a new folder beside `destination`, then replaces `destination` with it: a failure
 * leaves the previous folder, never a partial one. A destination that already holds the
 * same files is kept, so nothing that watches it rebuilds. */
export async function publishFolder(destination: string, fill: (stage: string) => Promise<void>) {
  await mkdir(dirname(destination), { recursive: true });
  const stage = `${destination}.building-${crypto.randomUUID()}`;
  await mkdir(stage);
  try {
    await fill(stage);
    const current = await digest(destination).catch(() => undefined);
    if (current === (await digest(stage))) return;
    await rm(destination, { recursive: true, force: true });
    await rename(stage, destination);
  } finally {
    await rm(stage, { recursive: true, force: true });
  }
}
/** Writes `content` to `path` only when it differs from the file there, so an unchanged
 * output keeps its modification time and nothing that watches it rebuilds. */
export async function writeIfChanged(path: string, content: string | Uint8Array): Promise<boolean> {
  const bytes = Buffer.from(content);
  if ((await readFile(path).catch(() => undefined))?.equals(bytes)) return false;
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, bytes);
  return true;
}
/** SHA-256 in hex. */
export const sha256 = (data: string | Uint8Array) => createHash("sha256").update(data).digest("hex");
/** Source-file identity must agree with the repository index when a file is staged. */
export function sourceBlobHash(data: Uint8Array, format: "sha1" | "sha256"): string {
  return createHash(format).update(`blob ${data.byteLength}\0`).update(data).digest("hex");
}
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
