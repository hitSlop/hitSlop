/** Crash contract for native storage: a write killed at any phase reopens old-or-new, never torn. */
import { strict as assert } from "node:assert";
import { mkdtemp, mkdir, writeFile, rm, cp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const phases = [
  "hold",
  "append:uncommitted",
  "append:committed",
  "checkpoint:uncommitted",
  "checkpoint:committed",
];
const fixture = "tests/fixtures/4-1/document";

async function native(binary: string, args: string[]) {
  const child = Bun.spawn([binary, ...args], { stdout: "pipe", stderr: "pipe" });
  const [out, error, code] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  return { out, error, code };
}
async function title(binary: string, root: string) {
  const result = await native(binary, ["get", root]);
  assert.equal(result.code, 0, result.error);
  return String(JSON.parse(result.out).title);
}

export async function runCrashMatrix(hostCheck = false) {
  const binary = resolve("apps/apple/Packages/HitSlopApple/.build/debug/hitslop-native");
  const folder = await mkdtemp(join(tmpdir(), "hitslop-crash-"));
  const results: string[] = [];
  try {
    for (const phase of phases) {
      const root = join(folder, `${phase.replace(":", "-")}.slop`);
      await cp(fixture, root, { recursive: true });
      const seeded = await native(binary, [
        "apply", root, "--op", JSON.stringify({ type: "splice", path: ["title"], index: 0, delete: 0, insert: "Acknowledged " }),
      ]);
      assert.equal(seeded.code, 0, seeded.error);
      const marker = join(folder, `${phase.replace(":", "-")}.paused`);
      const child = Bun.spawn([binary, "storage-probe", root, phase, marker], { stdout: "ignore", stderr: "inherit" });
      try {
        const deadline = Date.now() + 10000;
        while (!(await Bun.file(marker).exists())) {
          if (Date.now() > deadline || child.exitCode !== null) throw new Error(`probe failed to reach ${phase}`);
          await Bun.sleep(20);
        }
        // Writer exclusion: no second owner while the probe holds the lock.
        assert.notEqual((await native(binary, ["get", root])).code, 0);
      } finally {
        child.kill("SIGKILL");
        await child.exited;
      }
      const after = await title(binary, root);
      assert.ok(after.startsWith(phase === "append:committed" || phase === "checkpoint:committed" ? "Crash edit Acknowledged" : "Acknowledged"), `${phase}: ${after}`);
      results.push(phase);
      console.log(`PASS ${phase}: recovery and writer exclusion`);
    }
    if (hostCheck) {
      // The real host owns a WebView and socket; acknowledge through the CLI, then kill it.
      const root = join(folder, "Host.slop");
      await cp(fixture, root, { recursive: true });
      const app = process.env.HITSLOP_APP_BINARY ?? resolve("generated/v1/app/hitSlop.app/Contents/MacOS/hitSlop");
      const host = Bun.spawn([app, root], { stdout: "ignore", stderr: "ignore" });
      try {
        const deadline = Date.now() + 15000;
        while (!(await Bun.file(join(root, "state/host.lock")).exists())) {
          if (Date.now() > deadline || host.exitCode !== null) throw new Error("Native host startup failed");
          await Bun.sleep(30);
        }
        const child = Bun.spawn(
          [process.execPath, "packages/cli/src/cli.ts", "apply", root, "--op",
            JSON.stringify({ type: "splice", path: ["title"], index: 0, delete: 0, insert: "Native acknowledged" })],
          { stdout: "pipe", stderr: "pipe", env: { ...process.env, HITSLOP_NATIVE_CLI: process.env.HITSLOP_NATIVE_CLI ?? binary } },
        );
        const [out, error, code] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited]);
        assert.equal(code, 0, error);
        assert.ok(JSON.parse(out).title.startsWith("Native acknowledged"));
      } finally {
        host.kill("SIGKILL");
        await host.exited;
      }
      assert.ok((await title(binary, root)).startsWith("Native acknowledged"));
      results.push("host:acknowledged-write-survives-death");
    }
    await mkdir(".hitslop/v1-evidence", { recursive: true });
    await writeFile(
      `.hitslop/v1-evidence/crash-native${hostCheck ? "-host" : ""}.json`,
      JSON.stringify({ passed: true, cases: results }, null, 2) + "\n",
    );
    return results;
  } finally {
    await rm(folder, { recursive: true, force: true });
  }
}
if (import.meta.main) await runCrashMatrix(process.argv.includes("--host"));
