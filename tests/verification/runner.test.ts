import { expect, test } from "bun:test";
import { mkdtemp, rm, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { checkoutLease, retainReport, shardTests, assertShardComplete, testInventory } from "../../scripts/lib/verification";
import { exec } from "../../scripts/lib/test-process";
import { repository } from "../../scripts/lib/artifacts";

test("test discovery assigns each boundary once and refuses unknown or duplicate files", () => {
  const files = ["tests/packed/packed.test.ts", "packages/hitslop/tests/cli/build.test.ts", "packages/hitslop/tests/sdk/errors.test.ts", "tests/examples/quick-checklist.native.test.ts"];
  const groups = testInventory(files);
  expect(Object.values(groups).flat().sort()).toEqual(files.sort());
  expect(groups.cli).toEqual(["packages/hitslop/tests/cli/build.test.ts"]);
  expect(groups.native).toEqual(["tests/examples/quick-checklist.native.test.ts"]);
  expect(() => testInventory([files[0]!, files[0]!])).toThrow("Duplicate");
  expect(() => testInventory(["tests/forgotten/a.test.ts"])).toThrow("Unclassified");
});

test("a successful filtered retry retains the failed full-run report", async () => {
  const root = await mkdtemp(join(tmpdir(), "verify-report-"));
  try {
    const latest = join(root, "verify.json");
    await retainReport(join(root, "full"), latest, { passed: false, partial: false });
    await retainReport(join(root, "retry"), latest, { passed: true, partial: true });
    expect(JSON.parse(await readFile(join(root, "full/report.json"), "utf8"))).toEqual({ passed: false, partial: false });
    expect(JSON.parse(await readFile(latest, "utf8"))).toEqual({ passed: true, partial: true });
  } finally { await rm(root, { recursive: true, force: true }); }
});

test("a checkout permits only one verifier, then releases ownership", async () => {
  const root = await mkdtemp(join(tmpdir(), "verify-lock-"));
  let release: (() => void) | undefined;
  try {
    release = await checkoutLease(root);
    const result = await exec([process.execPath, "-e", `import {checkoutLease} from ${JSON.stringify(join(repository, "scripts/lib/verification.ts"))}; await checkoutLease(${JSON.stringify(root)});`]);
    expect(result.code).not.toBe(0);
    expect(result.stderr).toContain("Verification already running");
    release(); release = undefined;
    release = await checkoutLease(root);
  } finally { release?.(); await rm(root, { recursive: true, force: true }); }
}, 30_000);

test("Swift shard assignment is exhaustive, nonempty and rejects incomplete execution", () => {
  const ids = ["M.One/same()", "M.Two/same()", "M.Three/slow()"];
  const groups = shardTests(ids, 8, { "M.Three/slow()": 20 });
  expect(groups.flat().sort()).toEqual(ids.sort());
  expect(groups.every(group => group.length > 0)).toBe(true);
  expect(() => shardTests([], 3, {})).toThrow();
  expect(() => shardTests([ids[0]!, ids[0]!], 3, {})).toThrow();
  expect(() => assertShardComplete(ids, 2, 0)).toThrow("ran 2 of 3");
  expect(() => assertShardComplete(ids, 3, 1)).toThrow("exited 1");
  assertShardComplete(ids, 3, 0);
});
