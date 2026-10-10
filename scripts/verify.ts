/** The one test runner: the tiers a change touches, or every tier (docs/testing.md).
 *
 *   bun run verify                  the tiers whose inputs changed since they last passed here
 *   bun run verify --native         the same, with the native (macOS) tiers
 *   bun run verify --all            all ordinary tiers (with --native, also the macOS tiers)
 *   bun run verify --release        the release gate: shipping acceptance built from scratch, with a
 *                                   retained report (--built reuses builds, --skip-app)
 *   bun run verify TIER[,TIER] ...  those tiers only; the rest are the tier's own arguments
 *                                   (`verify rust store::`, `verify swift --filter Compat`)
 *
 *   --list        print the selection and why, without running it
 *   --json        machine-readable --list output (including native tiers on Linux)
 *   --base REF    instead, the tiers a change touches against REF's merge base (CI)
 *   --keep-going  run every selected tier even after one fails
 *   --no-build    trust the existing builds
 *   --ci          CI reporting (nextest's ci profile) */
import { tierInputs, sharedInputs, affectedTiers, touchesCompatibility, type TierName } from "./lib/verification-inputs";
import { appendFile, lstat, mkdir, readFile, readlink } from "node:fs/promises";
import { join } from "node:path";
import { availableParallelism, tmpdir } from "node:os";
import { mkdtempSync, rmSync } from "node:fs";
import { testInventory, checkoutLease, atomicJson, retainReport, changedPaths, type Preparation } from "./lib/verification";
import { exec as testExec, verificationAbort } from "./lib/test-process";
import { run } from "../packages/hitslop/src/cli/process";
import { repository, sha256, sourceBlobHash, useTestRegistry } from "./lib/artifacts";
import { debugHelper } from "./lib/native";
import { prepareNativeFixtures, stageNativeFixtures } from "./lib/native-fixtures";
import { swiftTests } from "./lib/swift-tests";
import { releases, replayedEntries, compatibilityMode } from "./compat/corpus";

type Build = "browser" | "web" | "native" | "templates" | "packages" | "app";
type Tier = {
  name: TierName;
  /** Runs the tier; `args` are a single-tier run's own arguments. */
  run: (args: string[], prepare: Preparation) => Promise<void>;
  /** Changed paths that select it. */
  inputs: RegExp[];
  /** The builds it needs, each done once per run, just before the first tier needing it. */
  needs?: Build[];
  /** A native tier: macOS only, selected with --native. */
  native?: boolean;
  /** Run nightly, in a release, or when named; never by a default local or PR run. */
  nightly?: boolean;
  /** Static checks run together, before the heavy tiers, with their output kept until done. */
  quick?: boolean;
};

// Options up to the first tier name are the runner's; what follows belongs to the tiers.
const all = process.argv.slice(2);
let split = all.length;
for (let index = 0; index < all.length; index++) {
  if (all[index] === "--base") index++;
  else if (!all[index]!.startsWith("-")) {
    split = index;
    break;
  }
}
const argv = all.slice(0, split);
const named = all[split]?.split(",");
const tierArgs = all.slice(split + 1);
const flag = (name: string) => argv.includes(name);
const option = (name: string) => (argv.includes(name) ? argv[argv.indexOf(name) + 1] : undefined);
if (flag("--json") && !flag("--list")) throw new Error("--json requires --list");
const release = flag("--release");
const ci = flag("--ci") || !!process.env.CI;
const releaseTag = process.env.HITSLOP_RELEASE_TAG?.replace(/^v/, "") || undefined;
// Nightly and release runs add expensive boundary cases, presentation, storage-growth
// budgets and broader randomized coverage. Full compatibility is independently selectable.
const nightly = release || process.env.HITSLOP_NIGHTLY === "1";
if (nightly) process.env.HITSLOP_NIGHTLY = "1";
else {
  process.env.HITSLOP_MODEL_SEEDS ||= "4";
  process.env.HITSLOP_MERGE_SEEDS ||= "8";
  process.env.HITSLOP_COMPAT_SEEDS ||= "1";
  process.env.HITSLOP_PUBLICATIONS_ROUNDS ||= "10";
}
const compatibilityBase = option("--base");
const sensitive = compatibilityBase && touchesCompatibility((await changedPaths(repository, compatibilityBase)).paths);
const compatibility = compatibilityMode({ ...process.env,
  HITSLOP_COMPAT_MODE: sensitive ? "full" : process.env.HITSLOP_COMPAT_MODE || "smoke",
});
const nextestFilters = (args: string[]) =>
  (nightly || args.length) && !args.includes("--ignore-default-filter") ? ["--ignore-default-filter"] : [];
process.env.HITSLOP_COMPAT_MODE = compatibility;
// Never let an inherited entry restriction weaken the selected coverage or its cache key.
if (compatibility === "full") delete process.env.HITSLOP_COMPAT_ENTRIES;
else process.env.HITSLOP_COMPAT_ENTRIES = replayedEntries(await releases(), false).map(({ name }) => name).join(",");
// A release builds, and so tests, what ships: the `dist` Cargo profile (scripts/build/core.ts).
if (release) process.env.HITSLOP_CARGO_PROFILE = "dist";
const scratch = mkdtempSync(join(tmpdir(), "hitslop-verify-"));
process.env.HITSLOP_TEST_REGISTRY ||= join(scratch, "registry");
process.env.TMPDIR = scratch;
process.once("exit", () => rmSync(scratch, { recursive: true, force: true }));
useTestRegistry();
const runId = `${new Date().toISOString().replace(/[:.]/g, "-")}-${process.pid}`;
let runDirectory = "";
const commands: { command: string[]; log: string; seconds: number; code: number }[] = [];
let commandNumber = 0;
async function logged(command: string[], options: { env?: Record<string, string | undefined>; cwd?: string; echo?: boolean } = {}) {
  const log = `${String(++commandNumber).padStart(3, "0")}-${command[0]!.split("/").at(-1)}.log`;
  const started = performance.now();
  let code = 1;
  try {
    const result = await testExec(command, { cwd: options.cwd ?? repository, env: { ...environment, ...options.env },
      echo: options.echo, log: join(runDirectory, log), timeout: 30 * 60_000 });
    code = result.code;
    return result;
  } finally { commands.push({ command, log, seconds: (performance.now() - started) / 1000, code }); }
}
/** The environment tiers run in, as this runner started: what an in-process build changes
 * (Vite sets NODE_ENV=production, which turns off Svelte's hot reload) never reaches a test. */
const environment = { ...process.env };

/** `command`, its output shared with this process; a failure throws. */
async function sh(command: string[], options: { env?: Record<string, string | undefined>; cwd?: string } = {}) {
  const { code } = await logged(command, { ...options, echo: true });
  if (code) throw new Error(`${command.join(" ")} exited with ${code}`);
}
/** `command`, its output returned with its status: quick tiers print theirs when done. */
async function quiet(command: string[], options: { env?: Record<string, string | undefined> } = {}) {
  const { code, stdout, stderr } = await logged(command, options);
  const output = (stdout + stderr).trim();
  if (code) throw new Error(`${command.join(" ")} exited with ${code}${output ? `\n${output}` : ""}`);
  return output;
}

const inventory = testInventory(["packages/hitslop/tests/**/*.test.ts", "tests/**/*.test.ts", "examples/slops/**/*.test.ts"]
  .flatMap(pattern => [...new Bun.Glob(pattern).scanSync(repository)]));

/** `bun test` over `files`: positional `args` narrow them by path, options pass through. */
async function bunTest(files: string[], args: string[], env: Record<string, string | undefined> = {}) {
  const filters = args.filter((arg) => !arg.startsWith("-"));
  const options = args.filter((arg) => arg.startsWith("-"));
  const selected = filters.length ? files.filter((file) => filters.some((filter) => file.includes(filter))) : files;
  if (!selected.length) throw new Error(`No test files match ${filters.join(" ")}`);
  await sh([process.execPath, "test", ...(ci ? ["--reporter=junit", `--reporter-outfile=${join(runDirectory, `bun-${commandNumber + 1}.xml`)}`] : []), ...options, ...selected.map((file) => "./" + file)], { env });
}

const tiers: Tier[] = [
  { name: "compat", quick: true, inputs: tierInputs.compat, run: async () => console.log(await quiet([process.execPath, "scripts/compat/check.ts"])) },
  {
    name: "tooling", inputs: tierInputs.tooling,
    run: (args) => bunTest(inventory.tooling, args),
  },
  {
    name: "contracts",
    quick: true,
    inputs: tierInputs.contracts,
    run: async () => {
      console.log(await quiet([process.execPath, "scripts/build/generate.ts", "--check"]));
    },
  },
  {
    name: "types",
    quick: true,
    inputs: tierInputs.types,
    run: async () => {
      const output = await Promise.all([
        quiet([process.execPath, "node_modules/typescript/bin/tsc", "-p", "tsconfig.json"]),
        quiet([process.execPath, "scripts/templates/check.ts"]),
      ]);
      console.log(output.filter(Boolean).join("\n"));
    },
  },
  {
    name: "bun",
    needs: ["web"],
    inputs: tierInputs.bun,
    run: (args) => bunTest(inventory.bun, [`--parallel=${Math.min(4, availableParallelism())}`, ...args]),
  },
  {
    name: "cli", needs: ["web"],
    inputs: tierInputs.cli,
    run: (args) => bunTest(inventory.cli, ["--parallel=1", "--timeout=30000", ...args]),
  },
  {
    name: "rust",
    inputs: tierInputs.rust,
    // A full run compiles and lints first; a filtered one (`verify rust store::`)
    // is for iterating, so it runs only the tests.
    run: async (args, prepare) => {
      if (!args.length) await prepare("Rust compilation and lints", async () => {
        await quiet(["cargo", "clippy", "--locked", "--workspace", "--all-targets", "--", "-D", "warnings"]);
        // The WASM adapter's lints do not depend on the host: Linux and release runs check
        // them, so macOS runs skip three builds and Homebrew's clang.
        if (process.platform === "linux" || release) {
          const wasm = ["-p", "hitslop-core-wasm", "--target", "wasm32-unknown-unknown"];
          await quiet(["cargo", "clippy", "--locked", ...wasm, "--", "-D", "warnings"]);
          const llvm = process.platform === "darwin" ? "/opt/homebrew/opt/llvm/bin/" : "";
          for (const feature of ["browser", "evaluator"]) await quiet(["cargo", "clippy", "--locked", ...wasm, "--features", feature, "--", "-D", "warnings"], { env: {
            CC_wasm32_unknown_unknown: process.env.CC_wasm32_unknown_unknown || `${llvm}clang`,
            AR_wasm32_unknown_unknown: process.env.AR_wasm32_unknown_unknown || `${llvm}llvm-ar`,
          } });
        }
        await quiet(["cargo", "nextest", "run", "--locked", "--workspace", "--no-run"]);
      });
      await sh(["cargo", "nextest", "run", "--locked", "--workspace", ...(ci ? ["--profile", "ci"] : []), ...nextestFilters(args), ...args]);
      if (!args.length && process.platform === "linux") {
        await prepare("Linux Rust configurations", async () => {
          await quiet(["cargo", "clippy", "--locked", "-p", "hitslop-core", "--no-default-features", "--tests", "--", "-D", "warnings"]);
          await quiet(["cargo", "nextest", "run", "--locked", "-p", "slop-engine", "--features", "bundled-sqlite", "--no-run"]);
        });
        await sh(["cargo", "nextest", "run", "--locked", "-p", "slop-engine", "--features", "bundled-sqlite", ...(ci ? ["--profile", "ci"] : []), ...nextestFilters([])]);
      }
    },
  },
  {
    name: "landing",
    inputs: tierInputs.landing,
    run: async () => {
      await sh([process.execPath, "run", "check"], { cwd: join(repository, "apps/landing") });
      if (release) await sh([process.execPath, "run", "build"], { cwd: join(repository, "apps/landing") });
    },
  },
  {
    name: "packed",
    needs: ["packages"],
    // What the published packages ship: their sources, starter, skills and the page shell.
    inputs: tierInputs.packed,
    // A release installs the packages against the native helper too.
    run: (args) => bunTest(inventory.packed, args, release ? { HITSLOP_PACKED_NATIVE: "1" } : {}),
  },
  {
    name: "swift",
    native: true,
    needs: ["native"],
    inputs: tierInputs.swift,
    run: async (args, prepare) => {
      const fixtures = await prepare("Native fixtures", prepareNativeFixtures);
      // Benchmarks change the trial template's build stage before packing it.
      if (Object.keys(process.env).some((name) => name.startsWith("HITSLOP_BENCH"))) await stageNativeFixtures();
      await swiftTests({
        ...environment,
        HITSLOP_TEST_EVIDENCE: runDirectory,
        HITSLOP_NATIVE_CLI: process.env.HITSLOP_NATIVE_CLI ?? debugHelper,
        HITSLOP_PRESENTATION_FIXTURES: JSON.stringify(fixtures),
      }, args, prepare);
    },
  },
  {
    name: "app",
    native: true,
    needs: ["app"],
    // Built only for a release: the native tier then kills the real app (the host crash case).
    inputs: tierInputs.app,
    run: async () => {},
  },
  {
    // Playwright WebKit and Chrome: `slop dev`, EditableText and the unshipped browser host.
    name: "browser", native: true, nightly: true, needs: ["browser"], inputs: tierInputs.browser,
    run: (args) => bunTest(inventory.browser, args, { HITSLOP_TEST_EVIDENCE: runDirectory }),
  },
  {
    name: "native",
    native: true,
    needs: ["native"],
    inputs: tierInputs.native,
    run: async (args, prepare) => {
      await bunTest(inventory.native, args, {
        HITSLOP_NATIVE_CLI: process.env.HITSLOP_NATIVE_CLI ?? debugHelper,
        HITSLOP_TEST_EVIDENCE: runDirectory,
        // A release renders every bundled template and requires its frozen corpus entry.
        ...(release ? { HITSLOP_RENDER: "all" } : {}),
        ...(release && releaseTag ? { HITSLOP_COMPAT_RELEASE: releaseTag } : {}),
        ...(release && !flag("--skip-app") ? { HITSLOP_APP_BINARY: join(repository, "generated/app/hitSlop.app/Contents/MacOS/hitSlop") } : {}),
      });
    },
  },
];
const tierNames = tiers.map((tier) => tier.name);

/** Every file in the working tree, by content hash: Git's for unchanged tracked files, ours
 * for modified and untracked ones. Ignored build outputs are not inputs. */
async function snapshot(): Promise<Record<string, string>> {
  const git = (args: string[]) => run(["git", ...args], { cwd: repository }).then((out) => out.split("\0").filter(Boolean));
  const [indexed, modified, untracked, objectFormat] = await Promise.all([
    git(["ls-files", "-s", "-z"]), git(["diff", "--name-only", "-z"]),
    git(["ls-files", "--others", "--exclude-standard", "-z"]),
    run(["git", "rev-parse", "--show-object-format"], { cwd: repository }),
  ]);
  const format = objectFormat.trim();
  if (format !== "sha1" && format !== "sha256") throw new Error(`Unsupported Git object format: ${format}`);
  const files: Record<string, string> = {};
  for (const entry of indexed) {
    const [meta, path] = entry.split("\t");
    files[path!] = meta!.split(" ")[1]!;
  }
  for (const path of [...modified, ...untracked]) {
    const file = join(repository, path);
    // Git stores a symlink's target text, not the contents of the linked file.
    const bytes = await lstat(file).then(info => info.isSymbolicLink()
      ? readlink(file).then(target => Buffer.from(target)) : readFile(file)).catch(() => undefined);
    if (bytes) files[path] = sourceBlobHash(bytes, format);
    else delete files[path];
  }
  return files;
}
/** Inputs of every tier: how checks run, and the toolchain. */
const inputsOf = (tier: Tier, files: Record<string, string>) =>
  Object.keys(files).filter((path) => [...sharedInputs, ...tier.inputs].some((pattern) => pattern.test(path))).sort();
// A base/named listing does not consult the local pass cache. A default listing must
// use the same toolchain identity as an actual run.
const usesRust = (tier: Tier) => tier.name === "rust" || tier.name === "contracts" || !!tier.needs?.length;
const candidates = named ? tiers.filter(tier => named.includes(tier.name))
  : tiers.filter(tier => (!tier.nightly || nightly) && (!tier.native || flag("--native") || release));
const listing = flag("--list") && (option("--base") || named || flag("--all") || release);
const toolchains = {
  rust: !listing && candidates.some(usesRust) ? await run(["rustc", "--version"]).then(s => s.trim()) : null,
  swift: !listing && process.platform === "darwin" && candidates.some(tier => tier.native && tier.name !== "browser")
    ? await run(["swift", "--version"]).then(s => s.trim()) : null,
};
const digestOf = (tier: Tier, files: Record<string, string>) =>
  sha256(JSON.stringify([Bun.version, { rust: usesRust(tier) ? toolchains.rust : null, swift: tier.native && tier.name !== "browser" ? toolchains.swift : null }, process.platform, process.arch, process.env.HITSLOP_CARGO_PROFILE || "release", nightly, ["swift", "native"].includes(tier.name) ? compatibility : null, inputsOf(tier, files).map((path) => [path, files[path]])]));
/** Paths among `tier`'s inputs that differ between two snapshots. */
const differences = (tier: Tier, before: Record<string, string>, after: Record<string, string>) =>
  [...new Set([...inputsOf(tier, before), ...inputsOf(tier, after)])].filter((path) => before[path] !== after[path]);
const summary = (paths: string[]) => (paths.length > 3 ? `${paths.slice(0, 3).join(", ")} and ${paths.length - 3} more` : paths.join(", "));

/** What passed on this machine: each tier's input digest, and the snapshot it passed on. */
type Passed = { tiers: Record<string, { digest: string; snapshot: string; at: string }>; snapshots: Record<string, Record<string, string>> };
const passedFile = join(repository, ".hitslop/verify/passed.json");
const releaseLease = flag("--list") ? undefined : await checkoutLease(join(repository, ".hitslop/verify"));
if (releaseLease) process.once("exit", releaseLease);
const history: Passed = await Bun.file(passedFile).json().catch(() => ({ tiers: {}, snapshots: {} }));
const current = await snapshot();
let recording = Promise.resolve();
/** Records that `tier` passed on the inputs it started with; writes one at a time. */
function recordPass(tier: Tier) {
  return (recording = recording.then(() => writePass(tier)));
}
async function writePass(tier: Tier) {
  const id = sha256(JSON.stringify(current)).slice(0, 16);
  history.snapshots[id] = current;
  history.tiers[tier.name] = { digest: digestOf(tier, current), snapshot: id, at: new Date().toISOString() };
  const used = new Set(Object.values(history.tiers).map(({ snapshot }) => snapshot));
  for (const key of Object.keys(history.snapshots)) if (!used.has(key)) delete history.snapshots[key];
  await mkdir(join(repository, ".hitslop/verify"), { recursive: true });
  await atomicJson(passedFile, history);
}

type Selection = { tier: Tier; reason: string; args: string[] }[];
/** CI: the `candidates` a pull request or push touches, against its base `ref`. A change to
 * shared CI execution touches every tier. */
async function touched(ref: string, candidates: Tier[], args: string[]): Promise<{ selection: Selection; base: string }> {
  const { base, paths: changed } = await changedPaths(repository, ref);
  const selection = affectedTiers(changed, candidates.map(tier => tier.name)).map(({ name, paths }) => ({
    tier: candidates.find(tier => tier.name === name)!, reason: summary(paths), args,
  }));
  return { selection, base };
}
async function select(): Promise<{ selection: Selection; base?: string }> {
  const ref = option("--base");
  if (named) {
    const unknown = named.filter((name) => !tierNames.some(tier => tier === name));
    if (unknown.length) throw new Error(`Unknown tier ${unknown.join(", ")}; tiers: ${tierNames.join(", ")}`);
    const chosen = tiers.filter((tier) => named.includes(tier.name));
    // With a base, only those of the named tiers the change touches (CI's native job).
    if (ref) return touched(ref, chosen, tierArgs);
    return { selection: chosen.map((tier) => ({ tier, reason: "named", args: tierArgs })) };
  }
  const native = flag("--native") || release;
  const eligible = tiers.filter((tier) => (!tier.nightly || nightly) && (native || !tier.native) && (tier.name !== "app" || (release && !flag("--skip-app"))));
  if (flag("--all") || release) return { selection: eligible.map((tier) => ({ tier, reason: release ? "release" : "--all", args: [] })) };
  if (ref) return touched(ref, eligible, []);
  // Locally: every tier whose inputs changed since it last passed on this machine.
  const selection: Selection = [];
  for (const tier of eligible) {
    const record = history.tiers[tier.name];
    if (record?.digest === digestOf(tier, current)) continue;
    const before = record && history.snapshots[record.snapshot];
    const changed = before ? differences(tier, before, current) : [];
    selection.push({ tier, reason: !record ? "has not passed here" : changed.length ? `${summary(changed)} changed since it passed` : "the toolchain or verification configuration changed since it passed", args: [] });
  }
  return { selection };
}

const { selection, base } = await select();
if (flag("--list") || !selection.length) {
  if (flag("--json")) {
    console.log(JSON.stringify({ base: base ?? null, compatibility, tiers: selection.map(({ tier, reason }) => ({ name: tier.name, reason })) }));
    process.exit(0);
  }
  if (!selection.length)
    console.log(base ? `Nothing to verify: no tier's inputs changed against ${base}.` : "Nothing to verify: every tier already passed with these exact inputs. Run with --all to check everything anyway.");
  for (const { tier, reason, args } of selection) console.log(`${tier.name.padEnd(9)} ${reason}${args.length ? `  [${args.join(" ")}]` : ""}`);
  process.exit(0);
}
if (selection.some(({ tier }) => tier.native) && process.platform !== "darwin") throw new Error("Native tiers require macOS");

runDirectory = join(repository, ".hitslop/evidence/runs", runId);
await mkdir(runDirectory, { recursive: true });
const report = {
  runId, directory: runDirectory, command: process.argv.slice(2),
  configuration: { profile: process.env.HITSLOP_CARGO_PROFILE || "release", nightly, compatibility, bunWorkers: Math.min(4, availableParallelism()), cliWorkers: 1 },
  partial: tierArgs.length > 0,
  reusedBuild: flag("--no-build") || flag("--built"),
  certifiesDefaultRun: !tierArgs.length && !flag("--no-build") && !flag("--built"),
  inventory, commands, toolchains,
  cancelled: false,
  mode: release ? "release" : flag("--all") ? "all" : selection.every(({ reason }) => reason === "named") ? "named" : "affected",
  commit: (await run(["git", "rev-parse", "HEAD"], { cwd: repository })).trim(),
  dirty: (await run(["git", "status", "--porcelain"], { cwd: repository })).trim().length > 0,
  base: base ?? null,
  platform: process.platform,
  arch: process.arch,
  bun: Bun.version,
  ...(release ? { skippedApp: flag("--skip-app"), compatRelease: releaseTag ?? null } : {}),
  passed: false,
  builds: [] as { name: string; seconds: number }[],
  tiers: [] as { name: string; reason: string; code: number; seconds: number; buildSeconds: number; output?: string }[],
  error: undefined as string | undefined,
};

const done = new Map<Build, Promise<void>>();
/** Builds `name` once; a native build covers the web one. */
function build(name: Build): Promise<void> {
  if (flag("--no-build") || (flag("--built") && (name === "native" || name === "templates"))) return Promise.resolve();
  if (name === "web" && done.has("native")) return done.get("native")!;
  if (!done.has(name))
    done.set(name, (async () => {
      const started = performance.now();
      if (name === "web") {
        await sh([process.execPath, "scripts/build/core.ts", "--wasm"]);
        await sh([process.execPath, "scripts/build/shell.ts"]);
        await sh([process.execPath, "scripts/build/core.ts", "--engine"]);
      }
      if (name === "browser") {
        await sh([process.execPath, "scripts/build/browser.ts"]);
        await sh([process.execPath, "scripts/build/core.ts", "--engine"]);
        await sh([process.execPath, "scripts/build/shell.ts"]);
      }
      if (name === "native") await sh([process.execPath, "scripts/build/build.ts"]);
      if (name === "templates") await sh([process.execPath, "scripts/templates/build.ts"]);
      if (name === "packages") await sh([process.execPath, "scripts/build/packages.ts"]);
      if (name === "app") await sh([process.execPath, "scripts/build/apple.ts"]);
      report.builds.push({ name, seconds: (performance.now() - started) / 1000 });
    })());
  return done.get(name)!;
}

const seconds = (since: number) => (performance.now() - since) / 1000;
async function runTier({ tier, reason, args }: Selection[number], capture = false) {
  const started = performance.now();
  let testsStarted = started;
  let buildSeconds = 0;
  let built = false;
  let preparationSeconds = 0;
  const prepare: Preparation = async (name, action) => {
    const start = performance.now();
    try { return await action(); }
    finally {
      const duration = seconds(start);
      preparationSeconds += duration;
      report.builds.push({ name, seconds: duration });
    }
  };
  try {
    if (!capture) console.log(`\n▶ ${tier.name} (${reason})`);
    for (const need of tier.needs ?? []) await build(need);
    built = true;
    buildSeconds = seconds(started);
    testsStarted = performance.now();
    await tier.run(args, prepare);
    // A partial run (a filter, an option) proves nothing about the whole tier.
    if (!args.length && !flag("--no-build") && !flag("--built")) await recordPass(tier);
    report.tiers.push({ name: tier.name, reason, code: 0, seconds: seconds(testsStarted) - preparationSeconds, buildSeconds: buildSeconds + preparationSeconds });
    return true;
  } catch (error) {
    report.tiers.push({ name: tier.name, reason, code: 1, seconds: built ? seconds(testsStarted) - preparationSeconds : 0, buildSeconds: (built ? buildSeconds : seconds(started)) + preparationSeconds, output: String(error) });
    console.error(`✗ ${tier.name}: ${error instanceof Error ? error.message : error}`);
    return false;
  }
}

const started = performance.now();
let passed = true;
try {
  // Everything a release ships is built first, so tiers test the shipped artifacts.
  if (release) {
    await build("native");
    await build("templates");
  }
  const quick = selection.filter(({ tier }) => tier.quick);
  if (quick.length) {
    console.log(`▶ ${quick.map(({ tier }) => tier.name).join(", ")}`);
    passed = (await Promise.all(quick.map((entry) => runTier(entry, true)))).every(Boolean);
  }
  for (const entry of selection.filter(({ tier }) => !tier.quick)) {
    if (verificationAbort.signal.aborted) throw new Error("Verification cancelled; remaining tiers were not run");
    if (!passed && !flag("--keep-going")) break;
    passed = (await runTier(entry)) && passed;
  }
  report.passed = passed;
  report.certifiesDefaultRun &&= passed && !named;
} catch (error) {
  report.error = String(error);
  report.certifiesDefaultRun = false;
  passed = false;
} finally {
  report.cancelled = verificationAbort.signal.aborted;
  const evidence = join(repository, ".hitslop/evidence");
  await mkdir(evidence, { recursive: true });
  const file = join(evidence, release ? "release-check.json" : "verify.json");
  await retainReport(runDirectory, file, report);
  console.log("");
  for (const { name, seconds } of report.builds) console.log(`  build ${name.padEnd(9)} ${seconds.toFixed(1)}s`);
  for (const { name, code, seconds } of report.tiers)
    console.log(`${code ? "✗" : "✓"} ${name.padEnd(9)} ${seconds.toFixed(1)}s`);
  console.log(`${passed ? "Passed" : "Failed"} in ${seconds(started).toFixed(1)}s; report: ${file.slice(repository.length + 1)}`);
  if (process.env.GITHUB_STEP_SUMMARY) {
    await appendFile(process.env.GITHUB_STEP_SUMMARY, [
      `\n### Verification: ${passed ? "passed" : "failed"}\n`,
      `Compatibility coverage: ${compatibility}. Nightly checks: ${nightly ? "enabled" : "disabled"}.\n`,
      "| Tier | Result | Build/preparation | Tests |", "|---|---|---:|---:|",
      ...report.tiers.map(tier => `| ${tier.name} | ${tier.code ? "failed" : "passed"} | ${tier.buildSeconds.toFixed(1)}s | ${tier.seconds.toFixed(1)}s |`),
      `\nVerification wall time: ${seconds(started).toFixed(1)}s.\n`,
    ].join("\n"));
  }
  if (report.error) console.error(report.error);
}
process.exit(passed ? 0 : 1);
