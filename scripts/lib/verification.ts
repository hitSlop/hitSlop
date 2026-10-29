/** Verification bookkeeping; no imports that start builds or tests. */
import { Database } from "bun:sqlite";
import { mkdir, readFile, rename, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { run } from "../../packages/tinys/src/cli/process";

/** Include both sides of renames, deletions and unusual filenames. Missing bases fail closed. */
export async function changedPaths(cwd: string, ref: string) {
  const base = (await run(["git", "merge-base", "HEAD", ref], { cwd })).trim();
  const paths = (await run(["git", "diff", "--no-renames", "--name-only", "-z", base, "--"], { cwd })).split("\0").filter(Boolean);
  return { base, paths };
}

export type Preparation = <T>(name: string, action: () => Promise<T>) => Promise<T>;

export type TestTier = "tooling" | "bun" | "cli" | "render" | "browser" | "native" | "packed";
export function testInventory(files: string[]): Record<TestTier, string[]> {
  const groups: Record<TestTier, string[]> = { tooling: [], bun: [], cli: [], render: [], browser: [], native: [], packed: [] };
  for (const file of files.sort()) {
    if (file.startsWith("tests/examples/") || file.startsWith("tinys/"))
      throw new Error(`Per-example tests are not allowed: ${file}`);
    const tier = file.startsWith("tests/packed/") ? "packed"
      : file.endsWith(".browser.test.ts") ? "browser"
      : /^(tests\/native\/(?:render|landing)|packages\/tinys\/tests\/cli\/build)\.native\.test\.ts$/.test(file) ? "render"
      : file.endsWith(".native.test.ts") ? "native"
      : file.startsWith("tests/verification/") ? "tooling"
      : file.startsWith("packages/tinys/tests/cli/") ? "cli"
      : /^(packages\/tinys\/tests\/(sdk|shell)\/|tests\/release\/)/.test(file) ? "bun" : undefined;
    if (!tier) throw new Error(`Unclassified test: ${file}`);
    groups[tier].push(file);
  }
  return groups;
}

export async function atomicJson(path: string, value: unknown) {
  const temporary = `${path}.${process.pid}.${crypto.randomUUID()}.tmp`;
  await writeFile(temporary, JSON.stringify(value, null, 2) + "\n");
  await rename(temporary, path);
}

/** SQLite's OS-backed lock is released even if the runner is killed. This database is
 * verification bookkeeping only; it never opens a document or the live owner registry. */
export async function checkoutLease(directory: string) {
  await mkdir(directory, { recursive: true });
  const db = new Database(join(directory, "runner.sqlite"));
  try { db.exec("PRAGMA busy_timeout=0; BEGIN EXCLUSIVE"); }
  catch {
    db.close();
    const owner = await readFile(join(directory, "runner.json"), "utf8").catch(() => "unknown process");
    throw new Error(`Verification already running in this checkout (${owner.trim()}). Wait for it to finish, or use a separate checkout.`);
  }
  try { await atomicJson(join(directory, "runner.json"), { pid: process.pid, started: new Date().toISOString() }); }
  catch (error) { db.close(); throw error; }
  return () => { db.exec("ROLLBACK"); db.close(); };
}

export async function retainReport(directory: string, latest: string, report: unknown) {
  await mkdir(directory, { recursive: true });
  await atomicJson(join(directory, "report.json"), report);
  await atomicJson(latest, report);
}
