/** One serial Swift test process with isolated temporary files and writer registry. */
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { exec } from "./test-process";
import { type Preparation } from "./verification";
import { repository } from "./artifacts";

const packagePath = "apps/apple/Packages/HitSlopApple";

/** Swift Testing reports names with parentheses; refuse a green run with no tests. */
export function assertSwiftTestsRan(output: string) {
  if (![...output.matchAll(/Test run with (\d+) tests?/g)].some(match => Number(match[1]) > 0))
    throw new Error("Swift test run executed no tests");
}

export async function swiftTests(env: Record<string, string | undefined>, args: string[], prepare: Preparation = (_name, action) => action()) {
  let commandNumber = 0;
  const run = (command: string[], extra: Record<string, string> = {}) =>
    exec(command, { cwd: repository, env: { ...env, ...extra }, echo: true, timeout: 20 * 60_000,
      log: env.HITSLOP_TEST_EVIDENCE ? join(env.HITSLOP_TEST_EVIDENCE, `swift-${++commandNumber}.log`) : undefined });
  // Configured runs let SwiftPM compile the requested configuration itself.
  if (!args.length) await prepare("Swift test compilation", async () => {
    if ((await run(["swift", "build", "--build-tests", "--package-path", packagePath])).code) throw new Error("Swift test build failed");
  });
  const scratch = await mkdtemp(join(tmpdir(), "hitslop-swift-"));
  try {
    const result = await run(
      ["swift", "test", ...(!args.length ? ["--skip-build"] : []), "--no-parallel", "--package-path", packagePath, ...args],
      { TMPDIR: scratch + "/", HITSLOP_TEST_REGISTRY: join(scratch, "registry") },
    );
    if (result.code) throw new Error(`swift test exited with ${result.code}`);
    assertSwiftTestsRan(result.stdout + result.stderr);
  } finally {
    await rm(scratch, { recursive: true, force: true });
  }
}
