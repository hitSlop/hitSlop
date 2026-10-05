/** One release gate and a retained report, including failed stages. Does not publish. */
import { mkdir, writeFile } from "node:fs/promises";
import { exec, run } from "../packages/cli/src/process";
const directory = ".hitslop/evidence";
await mkdir(directory, { recursive: true });
const commit = (await run(["git", "rev-parse", "HEAD"], { failure: "Cannot identify release commit" })).trim();
const dirty = (await run(["git", "status", "--porcelain"], { failure: "Cannot identify working tree state" })).trim().length > 0;
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
// A tag build must ship its own frozen compatibility corpus entry, and run every frozen
// release's CLI against this helper.
const version = process.env.HITSLOP_RELEASE_TAG?.replace(/^macos-v/, "");
report.compatRelease = version ?? null;
try {
  if (process.platform !== "darwin") throw new Error("The complete release gate requires macOS");
  for (const command of [
    "hygiene",
    ...(process.argv.includes("--built") ? [] : ["build", "build:templates"]),
    "check",
    "check:built",
    "test",
    "swift:test",
    "test:native",
    "test:render",
    "packages:pack",
    version ? `test:compat --release ${version} --installed` : "test:compat",
    "test:native-helper",
    "test:packed --native",
    "landing:check",
    "landing:build",
    // The tag workflow accepts its signed Release app instead of building a Debug one.
    ...(process.argv.includes("--skip-app") ? [] : ["apple:build", "test:native-crash"]),
  ]) {
    const start = performance.now();
    const { code } = await exec([process.execPath, "run", ...command.split(" ")], { inherit: ["stdout", "stderr"] });
    stages.push({ command, code, seconds: (performance.now() - start) / 1000 });
    if (code) throw new Error(`Release stage failed: ${command}`);
  }
  report.passed = true;
} catch (error) {
  report.error = String(error);
  process.exitCode = 1;
} finally {
  await writeFile(`${directory}/release-check.json`, JSON.stringify(report, null, 2) + "\n");
  console.log(`Release report: ${directory}/release-check.json`);
}
