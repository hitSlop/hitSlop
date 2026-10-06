/** The Swift package's tests as concurrent process shards. Each shard is its own process,
 * with its own main thread, temporary folder and writer-lock registry, so WebKit and AppKit
 * suites run side by side while each suite stays serialized within its shard. Shards are
 * balanced by the durations the previous run recorded; a test that ran in none of them
 * fails the run, so a filter can never skip one silently. */
import { mkdir, mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { exec } from "../../packages/hitslop/src/cli/process";
import { repository } from "./artifacts";

const packagePath = "apps/apple/Packages/HitSlopApple";
const timingsFile = join(repository, ".hitslop/verify/swift-timings.json");
/** Concurrent shards: three halved the suite's time, with no flakes under load; four were no faster. */
const shards = 3;

/** A test's name as Swift Testing reports it: `name(labels:)`, without module or suite. */
const nameOf = (id: string) => id.slice(id.lastIndexOf("/") + 1).replace(/^[^.(]*\./, "");
/** A filter matching exactly the test `id` (`Module.Suite/name(…)`); `$` never matches. */
const filterOf = (id: string) => id.slice(0, id.indexOf("(")).replace(/[.*+?^${}()|[\]\\]/g, "\\$&") + String.raw`\(`;

export async function swiftTests(env: Record<string, string | undefined>, args: string[]) {
  const run = (command: string[], extra: Record<string, string> = {}, inherit = true) =>
    exec(command, { cwd: repository, env: { ...env, ...extra }, inherit: inherit ? ["stdout", "stderr"] : [] });
  // A filtered or configured run (a benchmark, a compatibility recording) is one process.
  if (args.length) {
    const { code } = await run(["swift", "test", "--no-parallel", "--package-path", packagePath, ...args]);
    if (code) throw new Error(`swift test exited with ${code}`);
    return;
  }
  if ((await run(["swift", "build", "--build-tests", "--package-path", packagePath])).code) throw new Error("Swift test build failed");
  const listed = await run(["swift", "test", "list", "--skip-build", "--package-path", packagePath], {}, false);
  if (listed.code) throw new Error(`swift test list failed: ${listed.stderr}`);
  const ids = listed.stdout.split("\n").filter((line) => line.includes("("));
  const timings: Record<string, number> = await Bun.file(timingsFile).json().catch(() => ({}));
  const cost = (id: string) => timings[nameOf(id)] ?? 1;
  // Longest first, each to the least loaded shard.
  const groups = Array.from({ length: shards }, () => ({ ids: [] as string[], total: 0 }));
  for (const id of [...ids].sort((a, b) => cost(b) - cost(a))) {
    const group = groups.reduce((a, b) => (a.total <= b.total ? a : b));
    group.ids.push(id);
    group.total += cost(id);
  }
  const results = await Promise.all(
    groups.map(async (group, index) => {
      const scratch = await mkdtemp(join(tmpdir(), `hitslop-swift-${index}-`));
      try {
        const filters = group.ids.flatMap((id) => ["--filter", filterOf(id)]);
        const result = await run(
          ["swift", "test", "--skip-build", "--no-parallel", "--package-path", packagePath, ...filters],
          { TMPDIR: scratch + "/", HITSLOP_TEST_REGISTRY: join(scratch, "registry") },
          false,
        );
        const ran = [...result.stdout.matchAll(/Test run with (\d+) tests?/g)].reduce((sum, match) => sum + Number(match[1]), 0);
        console.log(`Swift shard ${index + 1}/${shards}: ${ran} of ${group.ids.length} tests, exit ${result.code}`);
        return { ...result, ran, expected: group.ids.length };
      } finally {
        await rm(scratch, { recursive: true, force: true });
      }
    }),
  );
  for (const { stdout } of results)
    for (const [, name, seconds] of stdout.matchAll(/Test (\S+\)) passed after ([0-9.]+) seconds/g)) timings[name!] = Number(seconds);
  await mkdir(join(repository, ".hitslop/verify"), { recursive: true });
  await Bun.write(timingsFile, JSON.stringify(timings, null, 2) + "\n");
  const failed = results.filter(({ code, ran, expected }) => code || ran !== expected);
  for (const { stdout, stderr } of failed) console.log(stdout + stderr);
  if (failed.length)
    throw new Error(failed.map(({ code, ran, expected }) => `a shard exited ${code} after ${ran} of its ${expected} tests`).join("; "));
  console.log(`Swift tests: ${ids.length} in ${shards} shards`);
}
