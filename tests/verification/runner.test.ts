import { expect, test } from "bun:test";
import { mkdtemp, rm, readFile, mkdir, writeFile, rename } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { checkoutLease, retainReport, shardTests, assertShardComplete, testInventory, changedPaths } from "../../scripts/lib/verification";
import { tierInputs, affectedTiers, type TierName } from "../../scripts/lib/verification-inputs";
import { exec } from "../../scripts/lib/test-process";
import { repository } from "../../scripts/lib/artifacts";
import { verificationArgs } from "../../scripts/ci/select";

test("machine-readable selection lists native tiers without running tools or tests", async () => {
  const result = await exec([process.execPath, "scripts/verify.ts", "--list", "--json", "compat,rust,swift,native"], { cwd: repository });
  expect(result.code).toBe(0);
  expect(JSON.parse(result.stdout)).toEqual({
    base: null,
    tiers: ["compat", "rust", "swift", "native"].map(name => ({ name, reason: "named" })),
  });
});

const candidates = (Object.keys(tierInputs) as TierName[]).filter(name => name !== "app");
const affected = (...paths: string[]) => affectedTiers(paths, candidates).map(tier => tier.name);
test("attribution policy PR #5 does not build the product or allocate macOS", () => {
  expect(affected(
    ".claude/settings.json",
    ".github/workflows/block-ai-attribution.yml",
    "AGENTS.md",
    "docs/testing.md",
    "scripts/ci/attribution.ts",
    "tests/verification/attribution.test.ts",
  )).toEqual(["tooling", "types"]);
});

test("CI skips unrelated edits and selects affected boundaries", () => {
  expect(affected()).toEqual([]);
  expect(affected("docs/architecture.md")).toEqual([]);
  expect(affected(".gitignore", "AGENTS.md", ".vscode/launch.json", ".agents/skills/hitslop-native/SKILL.md")).toEqual([]);
  expect(affected("packages/hitslop/skills/hitslop/SKILL.md")).not.toContain("contracts");
  for (const path of ["tests/compat/dev/release.json", "scripts/compat/check.ts", "crates/hitslop-core/src/app/package_format_1.rs", "crates/hitslop-core/src/file/storage-1.sql"])
    expect(affected(path)).toContain("compat");
  expect(affected("apps/landing/src/routes/+page.svelte")).toEqual(["types", "landing"]);
  expect(affected("apps/apple/Packages/HitSlopApple/Tests/OwnerTests.swift")).toEqual(["swift", "native"]);
  expect(affected("crates/hitslop-core/src/store.rs")).toEqual(["contracts", "bun", "cli", "rust", "packed", "swift", "native"]);
  for (const path of ["packages/hitslop/src/sdk/context.ts", "packages/hitslop/src/shell/boot.js", "examples/slops/quick-checklist/App.svelte"])
    expect(affected(path)).toEqual(expect.arrayContaining(["bun", "cli", "packed", "swift", "native"]));
  for (const path of [".github/workflows/ci.yml", "bun.lock", "package.json", "scripts/lib/verification-inputs.ts"])
    expect(affected(path)).toEqual(candidates);
});

test("policy and verification tests select tooling while shared execution stays conservative", () => {
  for (const path of [".github/workflows/block-ai-attribution.yml", ".github/workflows/secret-scan.yml"])
    expect(affected(path)).toEqual(["tooling"]);
  for (const path of ["scripts/ci/attribution.ts", "tests/verification/runner.test.ts", "tests/verification/process.test.ts"])
    expect(affected(path)).toEqual(["tooling", "types"]);
  expect(affected(".github/actions/native-cache/action.yml")).toEqual(["swift", "native"]);
  for (const path of ["scripts/ci/select.ts", ".github/actions/prepare-checks/action.yml", "scripts/verify.ts", "scripts/lib/test-process.ts"])
    expect(affected(path)).toEqual(candidates);
});

test("nightly, manual and release branches select the full suite; ordinary changes use their base", () => {
  const full = ["--list", "--json", "--native", "--all"];
  for (const event of ["schedule", "workflow_dispatch"])
    expect(verificationArgs({ GITHUB_EVENT_NAME: event })).toEqual(full);
  expect(verificationArgs({ GITHUB_EVENT_NAME: "pull_request", GITHUB_BASE_REF: "release/1.0" })).toEqual(full);
  expect(verificationArgs({ GITHUB_EVENT_NAME: "push", GITHUB_REF_NAME: "release/1.0" })).toEqual(full);
  expect(verificationArgs({ GITHUB_EVENT_NAME: "pull_request", GITHUB_BASE_REF: "master" })).toEqual(["--list", "--json", "--native", "--base", "origin/master"]);
  const before = "a".repeat(40);
  expect(verificationArgs({ GITHUB_EVENT_NAME: "push", GITHUB_REF_NAME: "master", CHANGE_BASE: before })).toEqual(["--list", "--json", "--native", "--base", before]);
  expect(verificationArgs({ GITHUB_EVENT_NAME: "push", GITHUB_REF_NAME: "master", CHANGE_BASE: "0".repeat(40) })).toEqual(full);
  for (const env of [{}, { GITHUB_EVENT_NAME: "push", GITHUB_REF_NAME: "master" }, { GITHUB_EVENT_NAME: "pull_request" }])
    expect(() => verificationArgs(env)).toThrow();
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

test("test discovery assigns each boundary and refuses unclassified tests", () => {
  const files = ["tests/packed/packed.test.ts", "packages/hitslop/tests/cli/build.test.ts", "packages/hitslop/tests/sdk/errors.test.ts", "tests/examples/quick-checklist.native.test.ts", "tests/verification/runner.test.ts"];
  const groups = testInventory(files);
  expect(Object.values(groups).flat().sort()).toEqual(files.sort());
  expect(groups.cli).toEqual(["packages/hitslop/tests/cli/build.test.ts"]);
  expect(groups.native).toEqual(["tests/examples/quick-checklist.native.test.ts"]);
  expect(groups.tooling).toEqual(["tests/verification/runner.test.ts"]);
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
