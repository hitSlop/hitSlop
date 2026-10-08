/** The Swift package's tests as isolated process shards. Each shard is its own process,
 * with its own main thread, temporary folder and writer-lock registry, so WebKit and AppKit
 * suites stay serialized. SwiftPM invocations run sequentially because even --skip-build
 * opens the shared build database. Shards are
 * balanced by the durations the previous run recorded; a test that ran in none of them
 * fails the run, so a filter can never skip one silently. */
import { mkdir, mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { exec } from "./test-process";
import { availableParallelism } from "node:os";
import { assertShardComplete, shardTests, atomicJson, type Preparation } from "./verification";
import { repository } from "./artifacts";

const packagePath = "apps/apple/Packages/HitSlopApple";
const timingsFile = join(repository, ".hitslop/verify/swift-timings.json");
/** Preserve process isolation between groups without concurrent SwiftPM invocations. */
const shards = Math.min(3, availableParallelism());

/** A test's name as Swift Testing reports it: `name(labels:)`, without module or suite. */
const nameOf = (id: string) => id.slice(id.lastIndexOf("/") + 1).replace(/^[^.(]*\./, "");
/** A filter matching exactly the test `id` (`Module.Suite/name(…)`); `$` never matches. */
const filterOf = (id: string) => id.slice(0, id.indexOf("(")).replace(/[.*+?^${}()|[\]\\]/g, "\\$&") + String.raw`\(`;

export async function swiftTests(env: Record<string, string | undefined>, args: string[], prepare: Preparation = (_name, action) => action()) {
  let commandNumber = 0;
  const run = (command: string[], extra: Record<string, string> = {}, inherit = true) =>
    exec(command, { cwd: repository, env: { ...env, ...extra }, echo: inherit, timeout: 20 * 60_000,
      log: env.HITSLOP_TEST_EVIDENCE ? join(env.HITSLOP_TEST_EVIDENCE, `swift-${++commandNumber}.log`) : undefined });
  // A filtered or configured run (a benchmark, a compatibility recording) is one process.
  if (args.length) {
    const { code } = await run(["swift", "test", "--no-parallel", "--package-path", packagePath, ...args]);
    if (code) throw new Error(`swift test exited with ${code}`);
    return;
  }
  await prepare("Swift test compilation", async () => {
    if ((await run(["swift", "build", "--build-tests", "--package-path", packagePath])).code) throw new Error("Swift test build failed");
  });
  const listed = await run(["swift", "test", "list", "--skip-build", "--package-path", packagePath], {}, false);
  if (listed.code) throw new Error(`swift test list failed: ${listed.stderr}`);
  const ids = listed.stdout.split("\n").filter((line) => line.includes("("));
  const timings: Record<string, number> = await Bun.file(timingsFile).json().catch(() => ({}));
  const groups = shardTests(ids, shards, timings);
  if (env.HITSLOP_TEST_EVIDENCE) await atomicJson(join(env.HITSLOP_TEST_EVIDENCE, "swift-shards.json"), groups);
  const results: (Awaited<ReturnType<typeof run>> & { ran: number; expected: string[] })[] = [];
  for (const [index, group] of groups.entries()) {
    const scratch = await mkdtemp(join(tmpdir(), `hitslop-swift-${index}-`));
    try {
      const filters = group.flatMap((id) => ["--filter", filterOf(id)]);
      const result = await run(
        ["swift", "test", "--skip-build", "--no-parallel", "--package-path", packagePath, ...filters],
        { TMPDIR: scratch + "/", HITSLOP_TEST_REGISTRY: join(scratch, "registry") },
        false,
      );
      const ran = [...result.stdout.matchAll(/Test run with (\d+) tests?/g)].reduce((sum, match) => sum + Number(match[1]), 0);
      console.log(`Swift shard ${index + 1}/${groups.length}: ${ran} of ${group.length} tests, exit ${result.code}`);
      results.push({ ...result, ran, expected: group });
    } finally {
      await rm(scratch, { recursive: true, force: true });
    }
  }
  // Swift's console omits suite names. Only use a short name when it identifies
  // exactly one discovered test; store every estimate under that test's full ID.
  for (const { stdout } of results)
    for (const [, name, seconds] of stdout.matchAll(/Test (\S+\)) passed after ([0-9.]+) seconds/g)) {
      const matches = ids.filter(id => nameOf(id) === name);
      if (matches.length === 1) timings[matches[0]!] = Number(seconds);
    }
  await mkdir(join(repository, ".hitslop/verify"), { recursive: true });
  await Bun.write(timingsFile, JSON.stringify(timings, null, 2) + "\n");
  const failed = results.filter(({ code, ran, expected }) => code || ran !== expected.length);
  for (const { stdout, stderr } of failed) console.log(stdout + stderr);
  for (const { code, ran, expected } of results) assertShardComplete(expected, ran, code);
  console.log(`Swift tests: ${ids.length} in ${groups.length} shards`);
}
