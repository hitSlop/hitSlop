/** Crash contract for native storage: a helper killed at any point of an edit leaves the
 * document as it was or as edited, never torn, and its lock dies with it. An acknowledged
 * edit survives the death of the host that acknowledged it. */
import { strict as assert } from "node:assert";
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { documentFromStage } from "./fixture-documents";
import { engine } from "../packages/cli/src/engine";
import { useTestRegistry } from "./runtime-artifacts";

const fixture = "tests/fixtures/checklist/document";
const rounds = 24;

/** One helper request, killed after `killAfter` milliseconds unless it has answered. */
async function request(binary: string, body: Record<string, unknown>, killAfter?: number) {
  const child = Bun.spawn([binary, "request"], { stdin: "pipe", stdout: "pipe", stderr: "pipe" });
  child.stdin.write(JSON.stringify(body));
  await child.stdin.end();
  if (killAfter !== undefined) setTimeout(() => child.kill("SIGKILL"), killAfter);
  const [out, error, code] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  let reply: any;
  try { reply = JSON.parse(out); } catch {}
  return { reply, error, code };
}
async function title(binary: string, root: string) {
  const { reply, error } = await request(binary, { method: "get", documentPath: root });
  assert.ok(reply?.ok, error || reply?.error);
  return String(reply.state.state.value.title);
}

export async function runCrashMatrix(hostCheck = false) {
  useTestRegistry();
  const binary = resolve("apps/apple/Packages/HitSlopApple/.build/debug/hitslop-native");
  const folder = await mkdtemp(join(tmpdir(), "hitslop-crash-"));
  const results: string[] = [];
  try {
    const root = await documentFromStage(fixture, join(folder, "Killed.slop"), binary);
    let saved = await title(binary, root);
    let interrupted = 0;
    for (let round = 0; round < rounds; round++) {
      // Large enough that its save takes measurable time; the kill moves later each round.
      const value = `Round ${round} ${"x".repeat(16 * 1024)}`;
      const ops = JSON.stringify([{ type: "set", path: ["title"], value }]);
      const { reply, code } = await request(binary, { method: "batch", documentPath: root, ops }, round * 10);
      const acknowledged = code === 0 && reply?.ok === true;
      if (!acknowledged) interrupted++;
      // Reading the document also takes its lock: the killed helper's died with it.
      const after = await title(binary, root);
      if (acknowledged) assert.equal(after, value, `round ${round}: an acknowledged edit was lost`);
      else assert.ok(after === saved || after === value, `round ${round}: the document is neither as it was nor as edited`);
      saved = after;
    }
    assert.ok(interrupted > 0 && interrupted < rounds, `kills must land both before and after the reply (${interrupted}/${rounds})`);
    results.push(`helper:killed-mid-edit(${interrupted}/${rounds} interrupted)`);
    console.log(`PASS helper killed mid-edit: ${interrupted} of ${rounds} edits interrupted, none torn`);
    if (hostCheck) {
      // The real host owns a WebView and socket; acknowledge through the CLI, then kill it.
      const root = await documentFromStage(fixture, join(folder, "Host.slop"), binary);
      const app = process.env.HITSLOP_APP_BINARY ?? resolve("generated/app/hitSlop.app/Contents/MacOS/hitSlop");
      const host = Bun.spawn([app, root], { stdout: "ignore", stderr: "ignore" });
      try {
        const deadline = Date.now() + 15000;
        // The host is ready once it names its socket for commands.
        while (!JSON.parse(await engine(["inspect", root])).live) {
          if (Date.now() > deadline || host.exitCode !== null) throw new Error("Native host startup failed");
          await Bun.sleep(30);
        }
        const child = Bun.spawn(
          [process.execPath, "packages/cli/src/cli.ts", "apply", root, "--op",
            JSON.stringify({ type: "set", path: ["title"], value: "Native acknowledged" })],
          { stdout: "pipe", stderr: "pipe", env: { ...process.env, HITSLOP_NATIVE_CLI: process.env.HITSLOP_NATIVE_CLI ?? binary } },
        );
        const [error, code] = await Promise.all([new Response(child.stderr).text(), child.exited]);
        assert.equal(code, 0, error);
      } finally {
        host.kill("SIGKILL");
        await host.exited;
      }
      assert.equal(await title(binary, root), "Native acknowledged");
      results.push("host:acknowledged-write-survives-death");
    }
    await mkdir(".hitslop/evidence", { recursive: true });
    await writeFile(
      `.hitslop/evidence/crash-native${hostCheck ? "-host" : ""}.json`,
      JSON.stringify({ passed: true, cases: results }, null, 2) + "\n",
    );
    return results;
  } finally {
    await rm(folder, { recursive: true, force: true });
  }
}
if (import.meta.main) await runCrashMatrix(process.argv.includes("--host"));
