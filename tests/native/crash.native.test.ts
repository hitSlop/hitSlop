// Crash contract for native storage: a document engine killed at any point of an edit leaves
// the document as it was or as edited, never torn, and its lock dies with it. An acknowledged
// edit survives the death of the host that acknowledged it (with HITSLOP_APP_BINARY, the
// app to kill: `verify --release` builds one, release packaging names the signed app).
// Stress coverage beside the deterministic storage faults in the Rust suite.
import { afterAll, beforeAll, expect, test } from "bun:test";
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { debugHelper, documentFromStage, engineRequest } from "../../scripts/lib/native";
import { execute } from "../../packages/hitslop/src/cli/engine";
import { negotiate } from "../../packages/hitslop/src/cli/engine";
import { run } from "../../packages/hitslop/src/cli/process";
import { useTestRegistry } from "../../scripts/lib/artifacts";
useTestRegistry();

const fixture = "tests/fixtures/checklist/document";
const rounds = 24;
const helper = process.env.HITSLOP_NATIVE_CLI ?? debugHelper;
// The engine the CLI selects for that helper: the one beside it.
const documentEngine = await (await import("../../packages/hitslop/src/cli/engine")).findEngine();
const app = process.env.HITSLOP_APP_BINARY;
let folder: string;
beforeAll(async () => {
  folder = await mkdtemp(join(tmpdir(), "hitslop-crash-"));
});
afterAll(() => rm(folder, { recursive: true, force: true }));

/** One engine request, killed after `killAfter` milliseconds unless it has answered. */
async function request(body: Record<string, unknown>, killAfter: number) {
  const child = Bun.spawn(negotiate(documentEngine), { stdin: "pipe", stdout: "pipe", stderr: "pipe" });
  child.stdin.write(JSON.stringify(body));
  await child.stdin.end();
  setTimeout(() => child.kill("SIGKILL"), killAfter);
  const [out, code] = await Promise.all([new Response(child.stdout).text(), child.exited]);
  let reply: any;
  try {
    reply = JSON.parse(out);
  } catch {}
  return { reply, code };
}
async function title(root: string) {
  const { value } = (await engineRequest({ method: "get", documentPath: root }, { engine: documentEngine })).state;
  return String((value as { title: unknown }).title);
}
async function evidence(name: string, cases: string[]) {
  await mkdir(".hitslop/evidence", { recursive: true });
  await writeFile(`.hitslop/evidence/${name}.json`, JSON.stringify({ passed: true, cases }, null, 2) + "\n");
}

test("an engine killed at any point of an edit leaves the document as it was or as edited", async () => {
  const root = await documentFromStage(fixture, join(folder, "Killed.slop"), { engine: documentEngine });
  let saved = await title(root);
  let interrupted = 0;
  for (let round = 0; round < rounds; round++) {
    // Large enough that its save takes measurable time; the kill moves later each round.
    const value = `Round ${round} ${"x".repeat(16 * 1024)}`;
    const batch = { intents: [{ type: "set", path: ["title"], value }] };
    const { reply, code } = await request({ method: "batch", documentPath: root, batch }, round * 10);
    const acknowledged = code === 0 && reply?.ok === true;
    if (!acknowledged) interrupted++;
    // Reading the document also takes its lock: the killed engine's died with it.
    const after = await title(root);
    if (acknowledged) expect(after, `round ${round}: an acknowledged edit was lost`).toBe(value);
    else expect(after === saved || after === value, `round ${round}: neither as it was nor as edited`).toBe(true);
    saved = after;
  }
  // Kills must land both before and after the reply.
  expect(interrupted).toBeGreaterThan(0);
  expect(interrupted).toBeLessThan(rounds);
  await evidence("crash-native", [`engine:killed-mid-edit(${interrupted}/${rounds} interrupted)`]);
}, 120_000);

test.if(!!app)("an acknowledged edit survives the death of the host that acknowledged it", async () => {
  // The real host owns a WebView and socket; acknowledge through the CLI, then kill it.
  const root = await documentFromStage(fixture, join(folder, "Host.slop"), { engine: documentEngine });
  const host = Bun.spawn([app!, root], { stdout: "ignore", stderr: "ignore" });
  try {
    const deadline = Date.now() + 15000;
    // The host is ready once it names its socket for commands.
    while (!(await execute({ method: "inspect", file: root })) .info.live) {
      if (Date.now() > deadline || host.exitCode !== null) throw new Error("Native host startup failed");
      await Bun.sleep(30);
    }
    await run(
      [process.execPath, "packages/hitslop/src/cli/cli.ts", "apply", root, "--op", JSON.stringify({ type: "set", path: ["title"], value: "Native acknowledged" })],
      { env: { ...process.env, HITSLOP_NATIVE_CLI: helper } },
    );
  } finally {
    host.kill("SIGKILL");
    await host.exited;
  }
  expect(await title(root)).toBe("Native acknowledged");
  await evidence("crash-native-host", ["host:acknowledged-write-survives-death"]);
}, 120_000);
