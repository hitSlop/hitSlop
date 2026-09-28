// S3: one CLI edit path. Forward to a live owner or own a closed document, across
// startup/shutdown races and SIGKILLs. Counter increments make "applied exactly once"
// checkable: the final count must equal the number of acknowledged applies.
// Requires: swift build --package-path spikes/hitslop-core -c release
import { mkdtemp, rm, stat } from "node:fs/promises";
import { existsSync } from "node:fs";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import assert from "node:assert/strict";

const root = import.meta.dir;
const bin = join(root, ".build/out/Products/Release");
const evidence = resolve(root, "../../.hitslop/v1-evidence/hitslop-core");
const fixture = await Bun.file(join(root, "fixtures/checklist.json")).json();
const scratch = await mkdtemp(join(tmpdir(), "slop-races-"));
const schemaPath = join(scratch, "schema.json"), initialPath = join(scratch, "initial.json");
await Bun.write(schemaPath, JSON.stringify(fixture.schema));
await Bun.write(initialPath, JSON.stringify(fixture.initial));
const INC = JSON.stringify({ intents: [{ type: "increment", path: ["hits"], by: 1 }] });

type Result = { exit: number; out: any; err: string; ms: number };
async function cli(args: string[], env: Record<string, string> = {}): Promise<Result> {
  const t = performance.now();
  const p = Bun.spawn([join(bin, "slop-spike"), ...args], { stdout: "pipe", stderr: "pipe", env: { ...process.env, ...env } });
  const [out, err, exit] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text(), p.exited]);
  return { exit, out: out.trim() ? JSON.parse(out) : null, err, ms: performance.now() - t };
}
let serial = 0;
async function makeDoc() {
  const dir = join(scratch, `doc${serial++}`);
  const r = await cli(["create", dir, schemaPath, initialPath]);
  assert.equal(r.exit, 0, r.err);
  return dir;
}
async function hits(dir: string) {
  const r = await cli(["get", dir]);
  assert.equal(r.exit, 0, JSON.stringify(r));
  return JSON.parse(r.out.body).value as any;
}
const inode = async (dir: string) => (await stat(join(dir, "writer.lock"))).ino;

type Host = { proc: ReturnType<typeof Bun.spawn>; lines: string[]; waitFor: (line: string) => Promise<void> };
function host(dir: string, options: string[] = []): Host {
  const proc = Bun.spawn([join(bin, "owner-host"), dir, ...options], { stdout: "pipe", stderr: "inherit" });
  const lines: string[] = [];
  const waiters: [string, () => void][] = [];
  (async () => {
    const decoder = new TextDecoder();
    let buffer = "";
    for await (const chunk of proc.stdout as ReadableStream<Uint8Array>) {
      buffer += decoder.decode(chunk);
      let i;
      while ((i = buffer.indexOf("\n")) >= 0) {
        const line = buffer.slice(0, i);
        buffer = buffer.slice(i + 1);
        lines.push(line);
        for (const w of waiters.splice(0)) (line.startsWith(w[0]) ? w[1]() : waiters.push(w));
      }
    }
  })();
  return {
    proc, lines,
    waitFor: (prefix) => lines.some((l) => l.startsWith(prefix)) ? Promise.resolve() :
      new Promise((ok, fail) => { waiters.push([prefix, ok]); setTimeout(() => fail(new Error(`timeout waiting for ${prefix}: ${lines}`)), 10_000); }),
  };
}
const results: Record<string, any> = {};
async function scenario(name: string, body: () => Promise<any>) {
  const t = performance.now();
  const detail = await body();
  results[name] = { pass: true, seconds: +((performance.now() - t) / 1000).toFixed(2), ...detail };
  console.log(`pass  ${name}`, detail ? JSON.stringify(detail) : "");
}

await scenario("closed document: sequential CLI applies own in-process", async () => {
  const dir = await makeDoc(), ino = await inode(dir);
  for (let i = 0; i < 20; i++) {
    const r = await cli(["apply", dir, INC]);
    assert.equal(r.exit, 0); assert.equal(r.out.routed, "owned"); assert.equal(r.out.attempts, 1);
  }
  assert.equal((await hits(dir)).hits, 20);
  assert.ok(!existsSync(join(dir, "host.json")), "no discovery for a closed document");
  assert.equal(await inode(dir), ino);
});

await scenario("open document: CLI applies forward while the host edits", async () => {
  const dir = await makeDoc(), ino = await inode(dir);
  const h = host(dir, ["--edit-every", "5"]);
  await h.waitFor("READY");
  let acknowledged = 0;
  for (let round = 0; round < 6; round++) {
    const batch = await Promise.all(Array.from({ length: 5 }, () => cli(["apply", dir, INC])));
    for (const r of batch) { assert.equal(r.exit, 0, JSON.stringify(r)); assert.equal(r.out.routed, "forwarded"); acknowledged++; }
  }
  h.proc.kill("SIGTERM"); await h.proc.exited;
  const value = await hits(dir);
  assert.equal(value.hits, acknowledged);
  assert.ok(/^h+abc$/.test(value.title), `host edits durable: ${value.title}`);
  assert.equal(await inode(dir), ino);
  return { acknowledged, hostEdits: value.title.length - 3 };
});

await scenario("host startup: lock held before the socket listens", async () => {
  const dir = await makeDoc();
  const h = host(dir, ["--delay-listen", "400"]);
  await h.waitFor("LOCKED");
  const r = await cli(["apply", dir, INC]);
  assert.equal(r.exit, 0, JSON.stringify(r)); assert.equal(r.out.routed, "forwarded");
  assert.ok(r.out.attempts > 1, "retried until the socket was published");
  h.proc.kill("SIGTERM"); await h.proc.exited;
  assert.equal((await hits(dir)).hits, 1);
  return { attempts: r.out.attempts };
});

await scenario("host shutdown: CLI waits for the lock, then owns", async () => {
  const dir = await makeDoc(), ino = await inode(dir);
  const h = host(dir, ["--slow-close", "300"]);
  await h.waitFor("READY");
  assert.equal((await cli(["apply", dir, INC])).out.routed, "forwarded");
  h.proc.kill("SIGTERM");
  await h.waitFor("RELEASING");
  const r = await cli(["apply", dir, INC]);
  await h.proc.exited;
  assert.equal(r.exit, 0, JSON.stringify(r)); assert.equal(r.out.routed, "owned");
  assert.ok(r.out.attempts > 1);
  assert.equal((await hits(dir)).hits, 2);
  assert.equal(await inode(dir), ino);
  return { attempts: r.out.attempts };
});

await scenario("two CLIs at once on a closed document", async () => {
  const dir = await makeDoc();
  let routes: string[] = [];
  for (let i = 0; i < 20; i++) {
    const pair = await Promise.all([cli(["apply", dir, INC]), cli(["apply", dir, INC])]);
    for (const r of pair) { assert.equal(r.exit, 0, JSON.stringify(r)); routes.push(`${r.out.routed}/${r.out.attempts > 1 ? "retried" : "first"}`); }
  }
  assert.equal((await hits(dir)).hits, 40);
  return { routes: Object.fromEntries([...new Set(routes)].map((k) => [k, routes.filter((x) => x === k).length])) };
});

await scenario("SIGKILL the CLI owner mid-save (paused before COMMIT, 100x)", async () => {
  const dir = await makeDoc(), ino = await inode(dir);
  for (let i = 0; i < 100; i++) {
    const p = Bun.spawn([join(bin, "slop-spike"), "apply", dir, INC], { stdout: "pipe", stderr: "pipe", env: { ...process.env, SLOP_SPIKE_PAUSE_BEFORE_COMMIT: "1" } });
    const reader = (p.stderr as ReadableStream<Uint8Array>).getReader();
    let seen = "";
    while (!seen.includes("PAUSED")) { const { value, done } = await reader.read(); if (done) break; seen += new TextDecoder().decode(value); }
    assert.ok(seen.includes("PAUSED"));
    p.kill("SIGKILL"); await p.exited;
    const r = await cli(["get", dir]);
    assert.equal(r.out.attempts, 1, "the OS released the lock with the process");
    assert.equal(JSON.parse(r.out.body).value.hits, 0, "uncommitted save left no trace");
  }
  assert.equal(await inode(dir), ino);
});

await scenario("SIGKILL the CLI owner at random times (100x)", async () => {
  const dir = await makeDoc();
  let expected = 0, landed = 0;
  for (let i = 0; i < 100; i++) {
    const p = Bun.spawn([join(bin, "slop-spike"), "apply", dir, INC], { stdout: "pipe", stderr: "pipe" });
    await Bun.sleep(Math.random() * 14);
    p.kill("SIGKILL");
    await p.exited;
    const now = (await hits(dir)).hits;
    assert.ok(now === expected || now === expected + 1, `old or new, never torn: ${now} vs ${expected}`);
    if (now === expected + 1) landed++;
    expected = now;
  }
  return { killedBeforeCommit: 100 - landed, landedBeforeKill: landed };
});

await scenario("SIGKILL the host mid-edit: stale discovery is ignored", async () => {
  const dir = await makeDoc(), ino = await inode(dir);
  const h = host(dir, ["--edit-every", "2"]);
  await h.waitFor("READY");
  await Bun.sleep(150);
  h.proc.kill("SIGKILL"); await h.proc.exited;
  assert.ok(existsSync(join(dir, "host.json")), "the killed host left its discovery file");
  const r = await cli(["apply", dir, INC]);
  assert.equal(r.exit, 0); assert.equal(r.out.routed, "owned"); assert.equal(r.out.attempts, 1);
  const value = await hits(dir);
  assert.equal(value.hits, 1);
  assert.ok(/^h*abc$/.test(value.title));
  assert.equal(await inode(dir), ino);
  return { durableHostEdits: value.title.length - 3 };
});

await scenario("SIGKILL the host after applying, before replying", async () => {
  const dir = await makeDoc();
  const h = host(dir, ["--pause-before-reply", "5000"]);
  await h.waitFor("READY");
  const pending = cli(["apply", dir, INC]);
  await h.waitFor("APPLIED");
  h.proc.kill("SIGKILL"); await h.proc.exited;
  const r = await pending;
  assert.equal(r.exit, 3, JSON.stringify(r)); assert.equal(r.out.status, "unknown_outcome");
  assert.equal((await hits(dir)).hits, 1, "get reveals the real outcome");
});

// Cold start: process spawn → lock → load → open → apply → durable flush → exit.
const dist = resolve(root, "../engine-placement/dist");
const cold: any[] = [];
for (const rows of [1, 1000, 5000, 40000]) {
  const dir = join(scratch, `cold${rows}`);
  assert.equal((await cli(["seed", dir, join(dist, "checklist.schema.json"), join(dist, `checklist-${rows}.snapshot`)])).exit, 0);
  const view = JSON.parse((await cli(["get", dir])).out.body).value;
  const id = view.rows[view.rows.length - 1].$id;
  const wall: number[] = [], parts: any[] = [];
  for (let i = 0; i < 20; i++) {
    const r = await cli(["apply", dir, JSON.stringify({ intents: [{ type: "set", path: ["rows", { id }, "done"], value: i % 2 === 0 }] })]);
    assert.equal(r.exit, 0, JSON.stringify(r)); assert.equal(r.out.routed, "owned");
    wall.push(r.ms); parts.push(r.out);
  }
  const q = (xs: number[], p: number) => xs.toSorted((a, b) => a - b)[Math.min(xs.length - 1, Math.floor(xs.length * p))]!;
  const med = (k: string) => +q(parts.map((x) => x[k]), 0.5).toFixed(2);
  cold.push({ rows, wallP50: +q(wall, 0.5).toFixed(1), wallP95: +q(wall, 0.95).toFixed(1), inProcessP50: med("inProcessMS"), lockP50: med("lockMS"), openP50: med("openMS"), applyP50: med("applyMS"), flushP50: med("flushMS") });
  console.log("cold ", JSON.stringify(cold.at(-1)));
}
const at1k = cold.find((c) => c.rows === 1000)!;
results.coldStart = { runs: 20, target: "wall p95 <= 150 ms at 1k rows", pass: at1k.wallP95 <= 150, cells: cold };
await Bun.write(join(evidence, "cli-races.json"), JSON.stringify({ created: new Date().toISOString(), results }, null, 2) + "\n");
await rm(scratch, { recursive: true, force: true });
if (!results.coldStart.pass) { console.error("Cold-start target missed"); process.exitCode = 2; }
console.log("evidence", join(evidence, "cli-races.json"));
