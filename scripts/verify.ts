/** The one test runner: the tiers a change touches, or every tier (docs/testing.md).
 *
 *   bun run verify                  the tiers whose inputs changed since they last passed here
 *   bun run verify --native         the same, with the native (macOS) tiers
 *   bun run verify --all            every tier but the native ones (with --native, every tier)
 *   bun run verify --release        the release gate: every tier, built from scratch, with a
 *                                   retained report (--built reuses builds, --skip-app)
 *   bun run verify TIER[,TIER] ...  those tiers only; the rest are the tier's own arguments
 *                                   (`verify rust store::`, `verify swift --filter Compat`)
 *
 *   --list        print the selection and why, without running it
 *   --base REF    instead, the tiers a change touches against REF's merge base (CI)
 *   --keep-going  run every selected tier even after one fails
 *   --no-build    trust the existing builds
 *   --ci          CI reporting (nextest's ci profile) */
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { exec, run } from "../packages/hitslop/src/cli/process";
import { repository, sha256, useTestRegistry, verifyShellCopies } from "./lib/artifacts";
import { debugHelper } from "./lib/native";
import { prepareNativeFixtures, stageNativeFixtures } from "./lib/native-fixtures";
import { swiftFormat } from "./lib/swift-format";
import { swiftTests } from "./lib/swift-tests";
import { buildCoreWasm, buildEngine } from "./build/core";
import { buildShell } from "./build/shell";
import { buildNative } from "./build/build";
import { buildTemplates } from "./templates/build";

type Build = "web" | "native" | "templates" | "packages" | "app";
type Tier = {
  name: string;
  /** Runs the tier; `args` are a single-tier run's own arguments. */
  run: (args: string[]) => Promise<void>;
  /** Changed paths that select it. */
  inputs: RegExp[];
  /** The builds it needs, each done once per run, just before the first tier needing it. */
  needs?: Build[];
  /** A native tier: macOS only, selected with --native. */
  native?: boolean;
  /** Static checks run together, before the heavy tiers, with their output kept until done. */
  quick?: boolean;
  /** Seconds a run should take; a slower one warns, so slowdowns show early. */
  budget: number;
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
const release = flag("--release");
const ci = flag("--ci") || !!process.env.CI;
const releaseTag = process.env.HITSLOP_RELEASE_TAG?.replace(/^v/, "") || undefined;
useTestRegistry();
// A release builds, and so tests, what ships: the `dist` Cargo profile (scripts/build/core.ts).
if (release) process.env.HITSLOP_CARGO_PROFILE = "dist";
/** The environment tiers run in, as this runner started: what an in-process build changes
 * (Vite sets NODE_ENV=production, which turns off Svelte's hot reload) never reaches a test. */
const environment = { ...process.env };

/** `command`, its output shared with this process; a failure throws. */
async function sh(command: string[], options: { env?: Record<string, string | undefined>; cwd?: string } = {}) {
  const { code } = await exec(command, { cwd: options.cwd ?? repository, env: { ...environment, ...options.env }, inherit: ["stdout", "stderr"] });
  if (code) throw new Error(`${command.join(" ")} exited with ${code}`);
}
/** `command`, its output returned with its status: quick tiers print theirs when done. */
async function quiet(command: string[]) {
  const { code, stdout, stderr } = await exec(command, { cwd: repository, env: environment });
  const output = (stdout + stderr).trim();
  if (code) throw new Error(`${command.join(" ")} exited with ${code}${output ? `\n${output}` : ""}`);
  return output;
}

/** Test files under the Bun runner: native ones (`*.native.test.ts`) or the rest. */
function bunTests(native: boolean): string[] {
  return ["packages/hitslop/tests/**/*.test.ts", "tests/{examples,native,release}/**/*.test.ts"]
    .flatMap((pattern) => [...new Bun.Glob(pattern).scanSync(repository)])
    .filter((file) => file.endsWith(".native.test.ts") === native)
    .sort();
}
/** `bun test` over `files`: positional `args` narrow them by path, options pass through. */
async function bunTest(files: string[], args: string[], env: Record<string, string | undefined> = {}) {
  const filters = args.filter((arg) => !arg.startsWith("-"));
  const options = args.filter((arg) => arg.startsWith("-"));
  const selected = filters.length ? files.filter((file) => filters.some((filter) => file.includes(filter))) : files;
  if (!selected.length) throw new Error(`No test files match ${filters.join(" ")}`);
  await sh([process.execPath, "test", ...options, ...selected.map((file) => "./" + file)], { env });
}

const rustInputs = [/^crates\//, /^Cargo\.(toml|lock)$/, /^rust-toolchain\.toml$/, /^rustfmt\.toml$/];
const nativeInputs = [
  ...rustInputs,
  /^apps\/apple\//,
  /^packages\/hitslop\/(src\/(sdk|shell|schema)|generated|acceptance)\//,
  /^tests\/(abi|presentation|fixtures|compat)\//,
  /^examples\/slops\//,
  /^scripts\/(lib|build|templates)\//,
];

const tiers: Tier[] = [
  { name: "hygiene", quick: true, budget: 5, inputs: [/./], run: async () => console.log(await quiet([process.execPath, "scripts/hygiene.ts"])) },
  {
    name: "contracts",
    quick: true,
    budget: 15,
    inputs: [
      /^packages\/hitslop\/(src\/schema|generated|acceptance|tests\/schema)\//,
      /^packages\/hitslop\/(skills\/|src\/cli\/(skills-build|app)\.ts$)/,
      /^\.agents\/skills\//,
      /^scripts\/build\/(generate|rust-contracts|swift-contracts|skills)\.ts$/,
      /^scripts\/build\/(acceptance|runner)\.ts$/,
      /^packages\/hitslop\/src\/(sdk|shell)\//,
      /\.generated\.(rs|swift)$/,
    ],
    run: async () => {
      const output = await Promise.all([
        quiet([process.execPath, "scripts/build/generate.ts", "--check"]),
        quiet([process.execPath, "scripts/build/skills.ts", "--check"]),
      ]);
      console.log(output.filter(Boolean).join("\n"));
    },
  },
  {
    name: "types",
    quick: true,
    budget: 20,
    inputs: [/\.(ts|svelte)$/, /(^|\/)tsconfig[^/]*\.json$/, /(^|\/)package\.json$/, /^bun\.lock$/],
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
    budget: 30,
    needs: ["web"],
    inputs: [...rustInputs, /^packages\//, /^tests\/(examples|fixtures|compat|release)\//, /^scripts\//, /^examples\/slops\//],
    run: (args) => bunTest(bunTests(false), ["--parallel", ...args]),
  },
  {
    name: "rust",
    budget: 60,
    inputs: [...rustInputs, /^packages\/hitslop\/(src\/schema|generated|acceptance|tests\/schema)\//, /^tests\/compat\//, /^\.config\/nextest\.toml$/],
    // A full run checks formatting and lints first; a filtered one (`verify rust store::`)
    // is for iterating, so it runs only the tests.
    run: async (args) => {
      if (!args.length) {
        await quiet(["cargo", "fmt", "--all", "--check"]);
        await quiet(["cargo", "clippy", "--locked", "--workspace", "--all-targets", "--", "-D", "warnings"]);
        const wasm = ["-p", "hitslop-core-wasm", "--target", "wasm32-unknown-unknown"];
        await quiet(["cargo", "clippy", "--locked", ...wasm, "--", "-D", "warnings"]);
      }
      await sh(["cargo", "nextest", "run", "--locked", "--workspace", ...(ci ? ["--profile", "ci"] : []), ...args]);
    },
  },
  {
    name: "landing",
    budget: 40,
    inputs: [/^apps\/landing\//],
    run: async () => {
      await sh([process.execPath, "run", "check"], { cwd: join(repository, "apps/landing") });
      if (release) await sh([process.execPath, "run", "build"], { cwd: join(repository, "apps/landing") });
    },
  },
  {
    name: "packed",
    budget: 60,
    needs: ["packages"],
    // What the published packages ship: their sources, starter, skills and the page shell.
    inputs: [/^packages\/hitslop\/(src|templates|skills)\//, /^packages\/[^/]+\/package\.json$/, /^scripts\/build\/(packages|engines|shell)\.ts$/, /^tests\/packed\//],
    // A release installs the packages against the native helper too.
    run: (args) => sh([process.execPath, "test", "./tests/packed/packed.test.ts", ...args], { env: release ? { HITSLOP_PACKED_NATIVE: "1" } : {} }),
  },
  {
    name: "swift",
    native: true,
    budget: 90,
    needs: ["native"],
    inputs: nativeInputs,
    // A full run checks formatting first; a filtered one (`verify swift --filter X`) is for
    // iterating, so it runs only the tests.
    run: async (args) => {
      if (!args.length) await swiftFormat("lint");
      const fixtures = await prepareNativeFixtures();
      // Benchmarks change the trial template's build stage before packing it.
      if (Object.keys(process.env).some((name) => name.startsWith("HITSLOP_BENCH"))) await stageNativeFixtures();
      await swiftTests({
        ...environment,
        HITSLOP_NATIVE_CLI: process.env.HITSLOP_NATIVE_CLI ?? debugHelper,
        HITSLOP_PRESENTATION_FIXTURES: JSON.stringify(fixtures),
      }, args);
    },
  },
  {
    name: "app",
    native: true,
    budget: 150,
    needs: ["app"],
    // Built only for a release: the native tier then kills the real app (the host crash case).
    inputs: [],
    run: async () => {},
  },
  {
    name: "native",
    native: true,
    budget: 120,
    needs: ["native"],
    inputs: [...nativeInputs, /^packages\/hitslop\/(src\/cli|shell)\//, /^tests\/(native|examples)\//, /\.native\.test\.ts$/, /^scripts\/compat\//],
    run: (args) =>
      bunTest(bunTests(true), args, {
        HITSLOP_NATIVE_CLI: process.env.HITSLOP_NATIVE_CLI ?? debugHelper,
        // A release renders every bundled template and requires its frozen corpus entry.
        ...(release ? { HITSLOP_RENDER: "all" } : {}),
        ...(release && releaseTag ? { HITSLOP_COMPAT_RELEASE: releaseTag } : {}),
        ...(release && !flag("--skip-app") ? { HITSLOP_APP_BINARY: join(repository, "generated/app/hitSlop.app/Contents/MacOS/hitSlop") } : {}),
      }),
  },
];
const tierNames = tiers.map((tier) => tier.name);

/** Every file in the working tree, by content hash: Git's for unchanged tracked files, ours
 * for modified and untracked ones. Ignored build outputs are not inputs. */
async function snapshot(): Promise<Record<string, string>> {
  const git = (args: string[]) => run(["git", ...args], { cwd: repository }).then((out) => out.split("\0").filter(Boolean));
  const [indexed, modified, untracked] = await Promise.all([git(["ls-files", "-s", "-z"]), git(["diff", "--name-only", "-z"]), git(["ls-files", "--others", "--exclude-standard", "-z"])]);
  const files: Record<string, string> = {};
  for (const entry of indexed) {
    const [meta, path] = entry.split("\t");
    files[path!] = meta!.split(" ")[1]!;
  }
  for (const path of [...modified, ...untracked]) {
    const bytes = await readFile(join(repository, path)).catch(() => undefined);
    if (bytes) files[path] = sha256(bytes);
    else delete files[path];
  }
  return files;
}
/** Inputs of every tier: how checks run, and the toolchain. */
const shared = [/^scripts\/verify\.ts$/, /^package\.json$/, /^bun\.lock$/, /^rust-toolchain\.toml$/];
const inputsOf = (tier: Tier, files: Record<string, string>) =>
  Object.keys(files).filter((path) => [...shared, ...tier.inputs].some((pattern) => pattern.test(path))).sort();
const digestOf = (tier: Tier, files: Record<string, string>) =>
  sha256(JSON.stringify([Bun.version, process.arch, inputsOf(tier, files).map((path) => [path, files[path]])]));
/** Paths among `tier`'s inputs that differ between two snapshots. */
const differences = (tier: Tier, before: Record<string, string>, after: Record<string, string>) =>
  [...new Set([...inputsOf(tier, before), ...inputsOf(tier, after)])].filter((path) => before[path] !== after[path]);
const summary = (paths: string[]) => (paths.length > 3 ? `${paths.slice(0, 3).join(", ")} and ${paths.length - 3} more` : paths.join(", "));

/** What passed on this machine: each tier's input digest, and the snapshot it passed on. */
type Passed = { tiers: Record<string, { digest: string; snapshot: string; at: string }>; snapshots: Record<string, Record<string, string>> };
const passedFile = join(repository, ".hitslop/verify/passed.json");
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
  await writeFile(passedFile, JSON.stringify(history) + "\n");
}

async function mergeBase(ref: string): Promise<string> {
  const { code, stdout } = await exec(["git", "merge-base", "HEAD", ref], { cwd: repository });
  if (code) throw new Error(`Cannot compare with ${ref}`);
  return stdout.trim();
}

type Selection = { tier: Tier; reason: string; args: string[] }[];
/** CI: the `candidates` a pull request or push touches, against its base `ref`. A change to
 * CI itself touches every tier. */
async function touched(ref: string, candidates: Tier[], args: string[]): Promise<{ selection: Selection; base: string }> {
  const base = await mergeBase(ref);
  const changed = (await run(["git", "diff", "--name-only", base], { cwd: repository })).split("\n").filter(Boolean);
  const selection = candidates.flatMap((tier) => {
    const hits = changed.filter((path) => [/^\.github\//, ...shared, ...tier.inputs].some((pattern) => pattern.test(path)));
    return hits.length ? [{ tier, reason: summary(hits), args }] : [];
  });
  return { selection, base };
}
async function select(): Promise<{ selection: Selection; base?: string }> {
  const ref = option("--base");
  if (named) {
    const unknown = named.filter((name) => !tierNames.includes(name));
    if (unknown.length) throw new Error(`Unknown tier ${unknown.join(", ")}; tiers: ${tierNames.join(", ")}`);
    const chosen = tiers.filter((tier) => named.includes(tier.name));
    // With a base, only those of the named tiers the change touches (CI's native job).
    if (ref) return touched(ref, chosen, tierArgs);
    return { selection: chosen.map((tier) => ({ tier, reason: "named", args: tierArgs })) };
  }
  const native = flag("--native") || release;
  const eligible = tiers.filter((tier) => (native || !tier.native) && (tier.name !== "app" || (release && !flag("--skip-app"))));
  if (flag("--all") || release) return { selection: eligible.map((tier) => ({ tier, reason: release ? "release" : "--all", args: [] })) };
  if (ref) return touched(ref, eligible, []);
  // Locally: every tier whose inputs changed since it last passed on this machine.
  const selection: Selection = [];
  for (const tier of eligible) {
    const record = history.tiers[tier.name];
    if (record?.digest === digestOf(tier, current)) continue;
    const before = record && history.snapshots[record.snapshot];
    const changed = before ? differences(tier, before, current) : [];
    selection.push({ tier, reason: !record ? "has not passed here" : changed.length ? `${summary(changed)} changed since it passed` : "the toolchain changed since it passed", args: [] });
  }
  return { selection };
}

const { selection, base } = await select();
if (selection.some(({ tier }) => tier.native) && process.platform !== "darwin") throw new Error("Native tiers require macOS");
if (flag("--list") || !selection.length) {
  if (!selection.length)
    console.log(base ? `Nothing to verify: no tier's inputs changed against ${base}.` : "Nothing to verify: every tier already passed with these exact inputs. Run with --all to check everything anyway.");
  for (const { tier, reason, args } of selection) console.log(`${tier.name.padEnd(9)} ${reason}${args.length ? `  [${args.join(" ")}]` : ""}`);
  process.exit(0);
}

const report = {
  mode: release ? "release" : flag("--all") ? "all" : selection.every(({ reason }) => reason === "named") ? "named" : "affected",
  commit: (await run(["git", "rev-parse", "HEAD"], { cwd: repository })).trim(),
  dirty: (await run(["git", "status", "--porcelain"], { cwd: repository })).trim().length > 0,
  base: base ?? null,
  platform: process.platform,
  arch: process.arch,
  bun: Bun.version,
  ...(release ? { reusedBuild: flag("--built"), skippedApp: flag("--skip-app"), compatRelease: releaseTag ?? null } : {}),
  passed: false,
  builds: [] as { name: Build; seconds: number }[],
  tiers: [] as { name: string; reason: string; code: number; seconds: number; budget: number; output?: string }[],
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
      if (name === "web") await Promise.all([buildCoreWasm().then(buildShell), buildEngine()]);
      if (name === "native") await buildNative();
      if (name === "templates") await buildTemplates();
      if (name === "packages") await sh([process.execPath, "scripts/build/packages.ts"]);
      if (name === "app") await sh([process.execPath, "scripts/build/apple.ts"]);
      report.builds.push({ name, seconds: (performance.now() - started) / 1000 });
    })());
  return done.get(name)!;
}

const seconds = (since: number) => (performance.now() - since) / 1000;
async function runTier({ tier, reason, args }: Selection[number], capture = false) {
  const started = performance.now();
  try {
    if (!capture) console.log(`\n▶ ${tier.name} (${reason})`);
    for (const need of tier.needs ?? []) await build(need);
    await tier.run(args);
    // A partial run (a filter, an option) proves nothing about the whole tier.
    if (!args.length) await recordPass(tier);
    report.tiers.push({ name: tier.name, reason, code: 0, seconds: seconds(started), budget: tier.budget });
    return true;
  } catch (error) {
    report.tiers.push({ name: tier.name, reason, code: 1, seconds: seconds(started), budget: tier.budget, output: String(error) });
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
    await verifyShellCopies();
  }
  const quick = selection.filter(({ tier }) => tier.quick);
  if (quick.length) {
    console.log(`▶ ${quick.map(({ tier }) => tier.name).join(", ")}`);
    passed = (await Promise.all(quick.map((entry) => runTier(entry, true)))).every(Boolean);
  }
  for (const entry of selection.filter(({ tier }) => !tier.quick)) {
    if (!passed && !flag("--keep-going")) break;
    passed = (await runTier(entry)) && passed;
  }
  report.passed = passed;
} catch (error) {
  report.error = String(error);
  passed = false;
} finally {
  const evidence = join(repository, ".hitslop/evidence");
  await mkdir(evidence, { recursive: true });
  const file = join(evidence, release ? "release-check.json" : "verify.json");
  await writeFile(file, JSON.stringify(report, null, 2) + "\n");
  console.log("");
  for (const { name, seconds } of report.builds) console.log(`  build ${name.padEnd(9)} ${seconds.toFixed(1)}s`);
  for (const { name, code, seconds, budget } of report.tiers)
    console.log(`${code ? "✗" : "✓"} ${name.padEnd(9)} ${seconds.toFixed(1)}s${seconds > budget ? `  (over its ${budget}s budget)` : ""}`);
  console.log(`${passed ? "Passed" : "Failed"} in ${seconds(started).toFixed(1)}s; report: ${file.slice(repository.length + 1)}`);
  if (report.error) console.error(report.error);
}
process.exit(passed ? 0 : 1);
