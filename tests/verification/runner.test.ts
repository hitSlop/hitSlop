import { expect, test } from "bun:test";
import { cp, mkdtemp, rm, readFile, readdir, mkdir, writeFile, rename } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { checkoutLease, retainReport, testInventory, changedPaths } from "../../scripts/lib/verification";
import { tierInputs, affectedTiers, type TierName } from "../../scripts/lib/verification-inputs";
import { exec } from "../../scripts/lib/test-process";
import { repository } from "../../scripts/lib/artifacts";
import { assertSwiftTestsRan } from "../../scripts/lib/swift-tests";
import { ciJobs, touchesNativeGate, verificationArgs } from "../../scripts/ci/select";

test("machine-readable selection lists native tiers without running tools or tests", async () => {
  const result = await exec([process.execPath, "scripts/verify.ts", "--list", "--json", "compat,rust,swift,native"], { cwd: repository,
  });
  expect(result.code).toBe(0);
  expect(JSON.parse(result.stdout)).toEqual({
    base: null,
    tiers: ["compat", "rust", "swift", "native"].map(name => ({ name, reason: "named" })),
  });
});

test("CI selection works before dependencies are installed", async () => {
  const root = await mkdtemp(join(tmpdir(), "hitslop-selection-clean-"));
  try {
    await cp(join(repository, "scripts"), join(root, "scripts"), { recursive: true });
    await mkdir(join(root, "packages/hitslop"), { recursive: true });
    await cp(join(repository, "packages/hitslop/package.json"), join(root, "packages/hitslop/package.json"));
    await cp(join(repository, "packages/hitslop/src"), join(root, "packages/hitslop/src"), { recursive: true });
    const initialized = await exec(["git", "init", "--quiet"], { cwd: root });
    expect(initialized.code).toBe(0);
    const env = { ...process.env, HITSLOP_NIGHTLY: "",
      GITHUB_EVENT_NAME: "workflow_dispatch", GITHUB_BASE_REF: "", CHANGE_BASE: "" };
    // Refuse auto-install so an eager build-tool import fails immediately, even offline.
    const listing = await exec([process.execPath, "--no-install", "scripts/verify.ts", "--list", "--json", "--native", "--all"], { cwd: root, env });
    expect(listing.code, listing.stderr).toBe(0);
    const selected = await exec([process.execPath, "--no-install", "scripts/ci/select.ts"], { cwd: root, env });
    expect(selected.code, selected.stderr).toBe(0);
    const result = JSON.parse(selected.stdout);
    expect(result.tiers).toEqual(JSON.parse(listing.stdout).tiers);
    expect(result.jobs.native).toBe("swift,native");
    expect(result.jobs.nativeRust).toBe("rust");
    expect(await readdir(root)).not.toContain("node_modules");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 30_000);

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
  for (const path of ["scripts/compat/required-history.json", "scripts/compat/check.ts", "crates/hitslop-core/src/app/package_format_1.rs", "crates/hitslop-core/src/file/storage-1.sql"])
    expect(affected(path)).toContain("compat");
  expect(affected("apps/landing/src/routes/+page.svelte")).toEqual(["types", "landing"]);
  expect(affected("apps/apple/Packages/HitSlopApple/Tests/OwnerTests.swift")).toEqual(["swift", "native"]);
  expect(affected("crates/hitslop-core/src/store.rs")).toEqual(["contracts", "bun", "cli", "render", "browser", "rust", "sharing", "packed", "swift", "native"]);
  for (const path of ["packages/hitslop/src/sdk/context.ts", "packages/hitslop/src/shell/boot.js"])
    expect(affected(path)).toEqual(expect.arrayContaining(["bun", "cli", "packed", "swift", "native"]));
  for (const path of [".github/workflows/ci.yml", "bun.lock", "package.json", "scripts/lib/verification-inputs.ts"])
    expect(affected(path)).toEqual(candidates);
});

test("policy and verification tests select tooling while shared execution stays conservative", () => {
  for (const path of [".github/workflows/block-ai-attribution.yml", ".github/workflows/secret-scan.yml"])
    expect(affected(path)).toEqual(["tooling"]);
  for (const path of ["scripts/ci/attribution.ts", "tests/verification/runner.test.ts", "tests/verification/process.test.ts"])
    expect(affected(path)).toEqual(["tooling", "types"]);
  expect(affected(".github/actions/native-cache/action.yml")).toEqual(["render", "swift", "native"]);
  for (const path of ["scripts/ci/select.ts", ".github/actions/prepare-checks/action.yml", "scripts/verify.ts", "scripts/lib/test-process.ts"])
    expect(affected(path)).toEqual(candidates);
});

test("nightly, manual and release branches select the full suite; ordinary changes use their base", () => {
  const full = ["--list", "--json", "--native", "--all"];
  for (const event of ["schedule", "workflow_dispatch"])
    expect(verificationArgs({ GITHUB_EVENT_NAME: event })).toEqual(full);
  expect(verificationArgs({ GITHUB_EVENT_NAME: "pull_request", GITHUB_BASE_REF: "hotfix/1.0" })).toEqual(full);
  expect(verificationArgs({ GITHUB_EVENT_NAME: "push", GITHUB_REF_NAME: "hotfix/1.0" })).toEqual(full);
  expect(verificationArgs({ GITHUB_EVENT_NAME: "pull_request", GITHUB_BASE_REF: "master" })).toEqual(["--list", "--json", "--native", "--base", "origin/master"]);
  const before = "a".repeat(40);
  expect(verificationArgs({ GITHUB_EVENT_NAME: "push", GITHUB_REF_NAME: "master", CHANGE_BASE: before })).toEqual(["--list", "--json", "--native", "--base", before]);
  expect(verificationArgs({ GITHUB_EVENT_NAME: "push", GITHUB_REF_NAME: "master", CHANGE_BASE: "0".repeat(40) })).toEqual(full);
  for (const env of [{}, { GITHUB_EVENT_NAME: "push", GITHUB_REF_NAME: "master" }, { GITHUB_EVENT_NAME: "pull_request" }])
    expect(() => verificationArgs(env)).toThrow();
});

test("required CI keeps document compatibility and packaging, without browser qualification or duplicated portable checks", () => {
  const jobs = (paths: string[], event = "pull_request") => ciJobs(affected(...paths), event);
  const core = jobs(["crates/hitslop-core/src/store.rs"]);
  expect(core.native).toBe("swift,native");
  expect(core.nativeRust).toBe("rust");
  expect(core.rust).toBe("rust");
  expect(core.fast.split(",")).toEqual(expect.arrayContaining(["contracts", "bun", "cli", "packed"]));
  expect(core.qualification).toBe("");
  const corpus = jobs(["scripts/compat/required-history.json"]);
  expect(corpus.fast.split(",")).toContain("compat");
  expect(corpus.native).toBe("swift,native");
  const none = { native: "", nativeRust: "", rust: "", deferred: "", qualification: "" };
  expect(jobs(["docs/testing.md"])).toEqual({ fast: "", ...none });
  for (const path of ["tests/native/render.native.test.ts", "tests/native/landing.native.test.ts", "packages/hitslop/tests/cli/build.native.test.ts"])
    expect(jobs([path])).toEqual({ fast: "types,render", ...none });
  expect(jobs(["packages/hitslop/tests/sdk/editable-text.browser.test.ts"])).toEqual({ fast: "types", ...none });
  expect(jobs(["packages/hitslop/src/browser/worker.ts"]).fast.split(",")).toContain("packed");
  for (const event of ["pull_request", "push", "schedule", "workflow_dispatch"]) {
    const all = ciJobs(candidates, event);
    for (const job of [all.fast, all.native, all.nativeRust, all.rust]) {
      expect(job.split(",")).not.toContain("browser");
    }
    expect(all.qualification).toBe(["schedule", "workflow_dispatch"].includes(event) ? "browser,cli,packed" : "");
  }
});

test("a pull request defers Swift/native tiers unless it affects a critical boundary or its tests", () => {
  const authoring = ["packages/hitslop/src/cli/authoring.ts"];
  expect(touchesNativeGate(authoring)).toBe(false);
  const deferred = ciJobs(affected(...authoring), "pull_request", touchesNativeGate(authoring));
  expect(deferred.native).toBe("");
  expect(deferred.deferred.split(",")).toContain("native");
  for (const path of ["crates/hitslop-core/src/file/open.rs", "apps/apple/Packages/HitSlopApple/Sources/HitSlopHost/SlopWindow.swift",
    "packages/hitslop/src/shell/owner.ts", "scripts/compat/required-history.json", "tests/native/crash.native.test.ts",
    "tests/apps/fixture-checklist/slop.ts", "scripts/lib/native.ts", "scripts/lib/native-fixtures.ts", "scripts/lib/swift-tests.ts",
    "packages/hitslop/src/sdk/index.ts"]) {
    expect(touchesNativeGate([path])).toBe(true);
    expect(ciJobs(affected(path), "pull_request", true).deferred).toBe("");
  }
});

test("host release selection excludes the independent website checks", async () => {
  const result = await exec([process.execPath, "scripts/verify.ts", "--release", "--list", "--json"], { cwd: repository });
  expect(result.code).toBe(0);
  expect(JSON.parse(result.stdout).tiers.map((tier: { name: string }) => tier.name)).not.toContain("landing");
  const ordinary = await exec([process.execPath, "scripts/verify.ts", "--list", "--json", "landing"], { cwd: repository });
  expect(ordinary.code).toBe(0);
  expect(JSON.parse(ordinary.stdout).tiers.map((tier: { name: string }) => tier.name)).toEqual(["landing"]);
});

test("test-only edits and build inputs still select their owning checks", () => {
  const cases: [string, TierName[]][] = [
    ["tests/packed/packed.test.ts", ["packed"]],
    ["packages/hitslop/tests/cli/build.test.ts", ["cli"]],
    ["packages/hitslop/tests/sdk/errors.test.ts", ["bun"]],
    ["tests/native/render.native.test.ts", ["render"]],
    ["tests/native/landing.native.test.ts", ["render"]],
    ["apps/landing/scripts/templates.ts", ["render"]],
    ["packages/hitslop/tests/cli/build.native.test.ts", ["render"]],
    ["packages/hitslop/tests/cli/source-fixture.ts", ["render"]],
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
  const files = ["tests/packed/packed.test.ts", "packages/hitslop/tests/cli/build.test.ts", "packages/hitslop/tests/sdk/errors.test.ts", "packages/hitslop/tests/sdk/editable-text.browser.test.ts", "tests/verification/runner.test.ts", "tests/native/render.native.test.ts", "tests/native/landing.native.test.ts", "packages/hitslop/tests/cli/build.native.test.ts"];
  const groups = testInventory(files);
  expect(Object.values(groups).flat().sort()).toEqual(files.sort());
  expect(groups.render).toEqual(["packages/hitslop/tests/cli/build.native.test.ts", "tests/native/landing.native.test.ts", "tests/native/render.native.test.ts"]);
  expect(groups.cli).toEqual(["packages/hitslop/tests/cli/build.test.ts"]);
  expect(groups.browser).toEqual(["packages/hitslop/tests/sdk/editable-text.browser.test.ts"]);
  expect(groups.tooling).toEqual(["tests/verification/runner.test.ts"]);
  expect(() => testInventory(["tests/forgotten/a.test.ts"])).toThrow("Unclassified");
  for (const file of ["tests/examples/one.test.ts", "tests/examples/two.native.test.ts", "tests/examples/three.browser.test.ts", "slops/one/ui.test.ts"])
    expect(() => testInventory([file])).toThrow("Per-example tests");
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

test("Swift success requires at least one executed test", () => {
  expect(() => assertSwiftTestsRan("Test run with 0 tests passed")).toThrow("no tests");
  expect(() => assertSwiftTestsRan("Build complete")).toThrow("no tests");
  assertSwiftTestsRan("Test run with 228 tests passed");
});

test("formatter and lint policy changes select the tier that enforces them", () => {
  for (const path of [".prettierrc.json", ".prettierignore", ".oxlintrc.json"])
    expect(affected(path)).toEqual(["types"]);
  for (const path of ["sdk/base.css", "shell/boot.js", "browser/index.html"])
    expect(affected(`packages/hitslop/src/${path}`)).toContain("types");
  expect(affected("rustfmt.toml")).toContain("rust");
  for (const path of ["apps/apple/.swift-format", "scripts/lib/swift-format.ts"]) {
    expect(affected(path)).toContain("swift");
    expect(touchesNativeGate([path])).toBe(true);
  }
});

// Ordinary slop changes never select infrastructure suites through a borrowed example.
test("example changes select authoring checks; infrastructure fixtures select their consumers", () => {
  for (const path of ["slops/one/App.svelte", "slops/one/styles.css", "slops/one/slop.ts"])
    expect(affected(path)).toEqual(["types"]);
  expect(affected("slops/one/README.md")).toEqual([]);
  expect(affected("tests/apps/document/App.svelte")).toEqual(expect.arrayContaining(["types", "cli", "browser", "swift", "native"]));
  expect(affected("packages/hitslop/tests/sdk/editable-text.browser.test.ts")).toEqual(["types", "browser"]);
  expect(affected("packages/hitslop/templates/checklist/App.svelte")).toEqual(expect.arrayContaining(["types", "cli", "browser", "packed"]));
});
