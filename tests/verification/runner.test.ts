import { expect, test } from "bun:test";
import { mkdtemp, rm, readFile, mkdir, writeFile, rename } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { checkoutLease, retainReport, shardTests, assertShardComplete, testInventory, changedPaths } from "../../scripts/lib/verification";
import { tierInputs, affectedTiers, type TierName } from "../../scripts/lib/verification-inputs";
import { exec } from "../../scripts/lib/test-process";
import { repository } from "../../scripts/lib/artifacts";

test("machine-readable selection lists native tiers without running tools or tests", async () => {
  const result = await exec([process.execPath, "scripts/verify.ts", "--list", "--json", "hygiene,rust,swift,native"], { cwd: repository });
  expect(result.code).toBe(0);
  expect(JSON.parse(result.stdout)).toEqual({
    base: null,
    tiers: ["hygiene", "rust", "swift", "native"].map(name => ({ name, reason: "named" })),
  });
});

const candidates = (Object.keys(tierInputs) as TierName[]).filter(name => name !== "app");
const affected = (...paths: string[]) => affectedTiers(paths, candidates).map(tier => tier.name);
test("CI selects only affected boundaries and always keeps hygiene", () => {
  expect(affected()).toEqual(["hygiene"]);
  expect(affected("docs/architecture.md")).toEqual(["hygiene"]);
  expect(affected("apps/landing/src/routes/+page.svelte")).toEqual(["hygiene", "types", "landing"]);
  expect(affected("apps/apple/Packages/HitSlopApple/Tests/OwnerTests.swift")).toEqual(["hygiene", "swift", "native"]);
  expect(affected("crates/hitslop-core/src/store.rs")).toEqual(["hygiene", "contracts", "bun", "cli", "rust", "packed", "swift", "native"]);
  for (const path of ["packages/hitslop/src/sdk/context.ts", "packages/hitslop/src/shell/boot.js", "examples/slops/quick-checklist/App.svelte"])
    expect(affected(path)).toEqual(expect.arrayContaining(["bun", "cli", "packed", "swift", "native"]));
  for (const path of [".github/workflows/ci.yml", "bun.lock", "package.json", "scripts/lib/verification-inputs.ts"])
    expect(affected(path)).toEqual(candidates);
});

test("test-only edits and build inputs still select their owning checks", () => {
  const cases: [string, TierName[]][] = [
    ["tests/packed/packed.test.ts", ["packed"]],
    ["packages/hitslop/tests/cli/build.test.ts", ["cli"]],
    ["packages/hitslop/tests/sdk/errors.test.ts", ["bun"]],
    ["tests/native/render.native.test.ts", ["native"]],
    ["tests/fixtures/schema.json", ["bun", "cli", "swift", "native"]],
    ["packages/hitslop/src/wire/engine.generated.ts", ["contracts", "swift", "native"]],
    ["scripts/build/core.ts", ["packed"]],
    ["Cargo.toml", ["contracts", "rust", "swift", "native"]],
  ];
  for (const [path, expected] of cases) expect(affected(path)).toEqual(expect.arrayContaining(expected));
});

test("Git selection includes both sides of renames, deleted files and fails on a missing base", async () => {
  const root = await mkdtemp(join(tmpdir(), "verify-selection-"));
  const git = async (...args: string[]) => {
    const result = await exec(["git", ...args], { cwd: root });
    if (result.code) throw new Error(result.stderr);
    return result.stdout.trim();
  };
  try {
    await git("init", "-q");
    await mkdir(join(root, "crates"));
    await writeFile(join(root, "crates/old.rs"), "old source\n");
    await writeFile(join(root, "deleted.ts"), "deleted source\n");
    await git("add", ".");
    await git("-c", "user.name=Test", "-c", "user.email=test@example.invalid", "commit", "-qm", "base");
    const base = await git("rev-parse", "HEAD");
    await rename(join(root, "crates/old.rs"), join(root, "new\nname.rs"));
    await rm(join(root, "deleted.ts"));
    await git("add", ".");
    expect((await changedPaths(root, base)).paths.sort()).toEqual(["crates/old.rs", "deleted.ts", "new\nname.rs"]);
    await expect(changedPaths(root, "missing-ref")).rejects.toThrow();
  } finally { await rm(root, { recursive: true, force: true }); }
});

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
