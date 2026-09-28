/** One crash contract, exercised through separate Bun and native storage adapters. */
import { strict as assert } from "node:assert";
import { mkdtemp, mkdir, writeFile, rm, cp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { Document } from "../../packages/document/src/document";
import { SQLiteStore } from "../../packages/document/test-support/sqlite";
import { MemoryStore } from "../../packages/document/src/memory";
import { defineDocument, fromDescriptor, s } from "../../packages/document/src/schema";
export const crashSchema = defineDocument({ title: s.text() });
export const crashInitial = { title: "Initial" };
const phases = [
  "hold",
  "append:uncommitted",
  "append:committed",
  "checkpoint:uncommitted",
  "checkpoint:committed",
];

export async function runCrashMatrix(adapters: ("bun" | "native")[], hostCheck = false) {
  const binary = resolve("apps/apple/Packages/HitSlopApple/.build/debug/hitslop-native");
  const folder = await mkdtemp(join(tmpdir(), "hitslop-crash-"));
  const results: string[] = [];
  try {
    for (const adapter of adapters)
      for (const phase of phases) {
        const root = join(folder, `${adapter}-${phase.replace(":", "-")}`);
        await mkdir(root);
        const seed = await Document.open(crashSchema, await SQLiteStore.open(root), crashInitial);
        seed.fields.title.replace("Acknowledged");
        await seed.flush();
        const snapshot = seed.exportSnapshot();
        const key = seed.key;
        await seed.close();
        const memory = new MemoryStore();
        await memory.checkpoint("0", snapshot, key);
        const staged = await Document.open(crashSchema, memory, crashInitial);
        const from = staged.version();
        staged.fields.title.replace("Crash edit");
        const bytes = phase.startsWith("append:") ? staged.exportUpdates(from) : snapshot;
        await staged.close();
        const payload = join(root, "payload.bin"),
          marker = join(root, "paused");
        await writeFile(payload, bytes);
        const args =
          adapter === "bun"
            ? [process.execPath, "scripts/v1/storage-child.ts", root, phase, marker]
            : [binary, "storage-probe", root, phase, marker, payload];
        const child = Bun.spawn(args, { stdout: "ignore", stderr: "inherit" });
        try {
          const deadline = Date.now() + 10000;
          while (!(await Bun.file(marker).exists())) {
            if (Date.now() > deadline || child.exitCode !== null)
              throw new Error(`${adapter} failed to reach ${phase}`);
            await Bun.sleep(20);
          }
          await assert.rejects(() => SQLiteStore.open(root), /live writer/);
        } finally {
          child.kill("SIGKILL");
          await child.exited;
        }
        const io = await SQLiteStore.open(root),
          disk = await io.load();
        const restored = await Document.open(crashSchema, io, crashInitial);
        assert.equal(
          restored.current.title,
          phase === "append:committed" ? "Crash edit" : "Acknowledged",
        );
        if (phase === "checkpoint:committed") assert.equal(disk.updates.length, 0);
        if (phase === "checkpoint:uncommitted") assert.ok(disk.updates.length > 0);
        await restored.close();
        results.push(`${adapter}:${phase}`);
        console.log(`PASS ${adapter} ${phase}: recovery and writer exclusion`);
      }
    if (hostCheck) {
      // The real host owns a WebView and socket; acknowledge through CLI, then kill it.
      const root = join(folder, "Host.slop");
      await cp("tests/compatibility/2-1/document", root, { recursive: true });
      const app =
        process.env.HITSLOP_APP_BINARY ??
        resolve("generated/v1/app/hitSlop.app/Contents/MacOS/hitSlop");
      const host = Bun.spawn([app, root], {
        stdout: "ignore",
        stderr: "ignore",
      });
      try {
        const deadline = Date.now() + 15000;
        while (!(await Bun.file(join(root, "state/host.lock")).exists())) {
          if (Date.now() > deadline || host.exitCode !== null)
            throw new Error("Native host startup failed");
          await Bun.sleep(30);
        }
        const child = Bun.spawn(
          [
            process.execPath,
            "packages/cli/src/cli.ts",
            "apply",
            root,
            "--op",
            JSON.stringify({ type: "text.replace", path: ["title"], value: "Native acknowledged" }),
          ],
          {
            stdout: "pipe",
            stderr: "pipe",
            // A packaged candidate routes through its own embedded helper.
            env: { ...process.env, HITSLOP_NATIVE_CLI: process.env.HITSLOP_NATIVE_CLI ?? binary },
          },
        );
        const [out, error, code] = await Promise.all([
          new Response(child.stdout).text(),
          new Response(child.stderr).text(),
          child.exited,
        ]);
        assert.equal(code, 0, error);
        assert.equal(JSON.parse(out).title, "Native acknowledged");
      } finally {
        host.kill("SIGKILL");
        await host.exited;
      }
      const reopened = await Document.open(
        fromDescriptor(await Bun.file(join(root, "state.schema.json")).json()),
        await SQLiteStore.open(root),
        await Bun.file(join(root, "initial.json")).json(),
      );
      assert.equal(reopened.current.title, "Native acknowledged");
      await reopened.close();
      results.push("host:acknowledged-write-survives-death");
    }
    await mkdir(".hitslop/v1-evidence", { recursive: true });
    await writeFile(
      `.hitslop/v1-evidence/crash-${adapters.join("-")}${hostCheck ? "-host" : ""}.json`,
      JSON.stringify({ passed: true, cases: results }, null, 2) + "\n",
    );
    return results;
  } finally {
    await rm(folder, { recursive: true, force: true });
  }
}
if (import.meta.main)
  await runCrashMatrix(
    process.argv.includes("--native") ? ["native"] : ["bun", "native"],
    process.argv.includes("--host"),
  );
