/** One release gate and a retained report, including failed stages. Does not publish or seal. */
import { mkdir, writeFile } from "node:fs/promises";
import { checkRuntime } from "./compatibility";
import { strict as assert } from "node:assert";
const directory = ".hitslop/v1-evidence";
await mkdir(directory, { recursive: true });
const git = Bun.spawn(["git", "rev-parse", "HEAD"], { stdout: "pipe" });
const commit = (await new Response(git.stdout).text()).trim();
if (await git.exited) throw new Error("Cannot identify release commit");
const status = Bun.spawn(["git", "status", "--porcelain"], { stdout: "pipe" });
const dirty = (await new Response(status.stdout).text()).trim().length > 0;
if (await status.exited) throw new Error("Cannot identify working tree state");
const report: Record<string, unknown> = {
  commit,
  dirty,
  reusedBuild: process.argv.includes("--built"),
  skippedApp: process.argv.includes("--skip-app"),
  platform: process.platform,
  arch: process.arch,
  bun: Bun.version,
  passed: false,
  stages: [],
};
const stages = report.stages as { command: string; code: number; seconds: number }[];
try {
  if (process.platform !== "darwin") throw new Error("The complete release gate requires macOS");
  for (const command of [
    "hygiene",
    ...(process.argv.includes("--built") ? [] : ["build", "build:templates"]),
    "check",
    "check:built",
    "check:sealed-templates",
    "test",
    "swift:test",
    "test:native",
    "test:render",
    "test:native-helper",
    "test:storage",
    "packages:pack",
    "test:packed --native",
    "landing:check",
    "landing:build",
    // The tag workflow accepts its signed Release app instead of building a Debug one.
    ...(process.argv.includes("--skip-app") ? [] : ["apple:build", "test:native-crash"]),
  ]) {
    const start = performance.now();
    const child = Bun.spawn([process.execPath, "run", ...command.split(" ")], {
      stdout: "inherit",
      stderr: "inherit",
    });
    const code = await child.exited;
    stages.push({ command, code, seconds: (performance.now() - start) / 1000 });
    if (code) throw new Error(`Release stage failed: ${command}`);
  }
  const first = await checkRuntime({ templates: false });
  const second = await checkRuntime({ templates: false });
  assert.deepEqual(
    second.results,
    first.results,
    "Logical compatibility state differs across identical replays",
  );
  report.compatibility = first;
  report.passed = true;
} catch (error) {
  report.error = String(error);
  process.exitCode = 1;
} finally {
  await writeFile(`${directory}/release-check.json`, JSON.stringify(report, null, 2) + "\n");
  console.log(`Release report: ${directory}/release-check.json`);
}
