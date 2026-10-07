/** Verification bookkeeping; no imports that start builds or tests. */
import { Database } from "bun:sqlite";
import { mkdir, readFile, rename, writeFile } from "node:fs/promises";
import { join } from "node:path";

export type Preparation = <T>(name: string, action: () => Promise<T>) => Promise<T>;

export type TestTier = "bun" | "cli" | "native" | "packed";
export function testInventory(files: string[]): Record<TestTier, string[]> {
  const groups: Record<TestTier, string[]> = { bun: [], cli: [], native: [], packed: [] };
  const seen = new Set<string>();
  for (const file of files.sort()) {
    if (seen.has(file)) throw new Error(`Duplicate test: ${file}`);
    seen.add(file);
    if (!file.endsWith(".test.ts")) throw new Error(`Unclassified test: ${file}`);
    const tier = file.startsWith("tests/packed/") ? "packed"
      : file.endsWith(".native.test.ts") ? "native"
      : file.startsWith("packages/hitslop/tests/cli/") ? "cli"
      : /^(packages\/hitslop\/tests\/(sdk|shell)\/|tests\/(examples|release|verification)\/)/.test(file) ? "bun" : undefined;
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

export function shardTests(ids: string[], count: number, timings: Record<string, number>) {
  if (!ids.length || new Set(ids).size !== ids.length) throw new Error("Swift discovery returned no tests or duplicate test identities");
  const groups = Array.from({ length: Math.max(1, Math.min(count, ids.length)) }, () => ({ ids: [] as string[], total: 0 }));
  for (const id of [...ids].sort((a, b) => (timings[b] ?? 1) - (timings[a] ?? 1) || a.localeCompare(b))) {
    const group = groups.reduce((a, b) => a.total <= b.total ? a : b);
    group.ids.push(id); group.total += timings[id] ?? 1;
  }
  return groups.map(group => group.ids);
}
export function assertShardComplete(expected: string[], ran: number, code: number) {
  if (code || ran !== expected.length) throw new Error(`Swift shard exited ${code}: ran ${ran} of ${expected.length} tests\n${expected.join("\n")}`);
}
