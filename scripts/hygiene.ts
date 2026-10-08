import { repository } from "./lib/artifacts";
import { releases } from "./compat/corpus";
import { verifyCorpus } from "./compat/integrity";
import { lstat, stat, realpath, readFile } from "node:fs/promises";
import { basename, dirname, extname, resolve } from "node:path";
import { exec } from "../packages/hitslop/src/cli/process";

async function command(
  argv: string[],
  cwd = repository,
  quiet = false,
  extraEnv: Record<string, string> = {},
): Promise<string> {
  const label = argv.join(" ");
  process.stdout.write(`→ ${label}\n`);
  const { code: exitCode, stdout, stderr } = await exec(argv, {
    cwd,
    inherit: quiet ? [] : ["stdout", "stderr"],
    env: { ...process.env, ...extraEnv },
  });
  if (exitCode !== 0) {
    if (stdout) process.stderr.write(stdout);
    if (stderr) process.stderr.write(stderr);
    throw new Error(`${label} failed with exit code ${exitCode}`);
  }
  return stdout;
}

async function gitFiles(): Promise<string[]> {
  const candidates = (
    await command(
      ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"],
      repository,
      true,
    )
  )
    .split("\0")
    .filter(Boolean);
  const existing = await Promise.all(
    candidates.map(async (path) => ((await pathExists(resolve(repository, path))) ? path : null)),
  );
  return existing.filter((path): path is string => path !== null);
}

export function assertTrackedHygiene(files: string[]): void {
  const failures: string[] = [];
  for (const path of files) {
    const name = path.split("/").at(-1) ?? path;
    const lower = name.toLowerCase();
    const example = lower.endsWith(".example");
    if ((lower.startsWith(".env") || lower.startsWith(".dev.vars")) && !example)
      failures.push(path);
    if (
      /^authkey_.*\.p8$/i.test(name) ||
      /\.(p8|p12|key|jks|keystore|mobileprovision)$/i.test(name)
    )
      failures.push(path);
    // Local history, working plans and inspiration must not enter a release checkout.
    if (
      /^(archive|spikes|plans|examples\/archive|apps\/promo\/inspo)\//.test(path) ||
      path === "SLOPS.todo"
    )
      failures.push(path);
  }
  if (failures.length)
    throw new Error(
      `Forbidden tracked artifacts:\n${[...new Set(failures)].map((path) => `  - ${path}`).join("\n")}`,
    );
}

export function assertNoGeneratedSource(files: string[]): void {
  const generated = files.filter(
    (path) =>
      // The page shell's static entry module is authored JavaScript.
      path !== "packages/hitslop/src/shell/boot.js" &&
      /^packages\/[^/]+\/src\//.test(path) &&
      /\.(?:d\.ts|js)$/.test(path),
  );
  if (generated.length)
    throw new Error(
      `Generated JavaScript/declarations found beside package source:\n${generated.map((path) => `  - ${path}`).join("\n")}`,
    );
}

async function checkIgnored(path: string, shouldIgnore: boolean): Promise<void> {
  const child = Bun.spawn(["git", "check-ignore", "--no-index", "-q", path], { cwd: repository });
  const ignored = (await child.exited) === 0;
  if (ignored !== shouldIgnore)
    throw new Error(`${path} should ${shouldIgnore ? "" : "not "}be ignored`);
}

async function assertTextHygiene(files: string[]): Promise<void> {
  const extensions = new Set([
    "",
    ".css",
    ".html",
    ".js",
    ".json",
    ".jsx",
    ".md",
    ".mdx",
    ".sh",
    ".slop",
    ".svelte",
    ".swift",
    ".toml",
    ".ts",
    ".tsx",
    ".txt",
    ".yaml",
    ".yml",
  ]);
  const privatePaths: string[] = [];
  const privateKeys: string[] = [];
  for (const path of files) {
    if (!extensions.has(extname(path)) && basename(path) !== ".gitignore")
      continue;
    if (!(await stat(resolve(repository, path))).isFile()) continue;
    const bytes = new Uint8Array(await Bun.file(resolve(repository, path)).arrayBuffer());
    // A .slop file is SQLite: its text is stored as UTF-8 among binary pages, so it is
    // scanned byte for byte. Extensionless bundled executables are not release text.
    const slop = extname(path) === ".slop";
    if (bytes.includes(0) && !slop) continue;
    const text = slop ? Buffer.from(bytes).toString("latin1") : new TextDecoder().decode(bytes);
    if (/\/Users\/|\/Volumes\//.test(text)) privatePaths.push(path);
    if (/BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY/.test(text)) privateKeys.push(path);
  }
  if (privatePaths.length)
    throw new Error(
      `Private absolute paths found in release files:\n${privatePaths.map((path) => `  - ${path}`).join("\n")}`,
    );
  if (privateKeys.length)
    throw new Error(
      `Private key material found in release files:\n${privateKeys.map((path) => `  - ${path}`).join("\n")}`,
    );
}

async function pathExists(path: string): Promise<boolean> {
  try {
    await lstat(path);
    return true;
  } catch {
    return false;
  }
}

export async function assertSkill(
  path: string,
  root = repository,
): Promise<void> {
  const actual = await realpath(resolve(root, path));
  const boundary = await realpath(root);
  if (!actual.startsWith(boundary + "/")) throw new Error(`Skill escapes repository: ${path}`);
  const text = await readFile(resolve(root, path), "utf8");
  const match = text.match(/^---\n([\s\S]*?)\n---\n/);
  if (!match) throw new Error(`${path} has no YAML frontmatter`);
  const keys = [...match[1]!.matchAll(/^([a-zA-Z0-9_-]+):/gm)].map((item) => item[1]);
  if (keys.join(",") !== "name,description")
    throw new Error(`${path} frontmatter must contain only name and description`);
}

const docsExcluded = /^(?:archive|docs\/evidence|_docs|examples\/archive)\//;
const packageNames = ["hitslop"] as const;

/** The published surface is small even though its sources share one package. */
async function assertPackageBoundaries(files: string[]) {
  const pkg = await Bun.file(resolve(repository, "packages/hitslop/package.json")).json();
  const workspace = await Bun.file(resolve(repository, "package.json")).json();
  const apple = await readFile(resolve(repository, "apps/apple/project.yml"), "utf8");
  if (pkg.version !== workspace.version || pkg.version !== apple.match(/MARKETING_VERSION: "([^"]+)"/)?.[1])
    throw new Error("Mac, workspace and hitslop must share one release version");
  // Rust exports and shared type aliases form the bottom layer. Neither may depend
  // on SDK, shell or CLI implementation.
  const layers: Record<string, number> = { schema: 0, wire: 0, sdk: 1, shell: 2, cli: 3 };
  const prefix = "packages/hitslop/src/";
  for (const path of files.filter(p => p.startsWith(prefix) && /\.(ts|js|svelte)$/.test(p))) {
    const layer = path.slice(prefix.length).split("/")[0]!;
    const content = await readFile(resolve(repository, path), "utf8");
    for (const [, specifier] of content.matchAll(/(?:from\s*|import\s*\(\s*|import\s*)["']([^"']+)["']/g)) {
      if (!specifier!.startsWith(".")) continue;
      const target = resolve(repository, dirname(path), specifier!);
      const root = resolve(repository, prefix) + "/";
      if (target.startsWith(root)) {
        const dependency = target.slice(root.length).split("/")[0]!;
        if ((layers[dependency] ?? -1) > (layers[layer] ?? -1))
          throw new Error(`Package dependency points inward: ${path} imports ${specifier}`);
      }
    }
  }
}

/** The released versions the docs may pin: each package's `version` in the tree. */
async function packageVersions(root: string): Promise<Record<string, string>> {
  const versions: Record<string, string> = {};
  for (const name of packageNames)
    versions[name] = JSON.parse(await readFile(resolve(root, `packages/${name}/package.json`), "utf8")).version;
  return versions;
}

/**
 * Docs stay current: a relative link in a tracked Markdown page resolves to a file or
 * directory in the tree (a Starlight page's trailing-slash URL is not a file), and a
 * pinned `@hitslop/{cli,document,schema}@X.Y.Z` is the version the tree is at.
 */
export async function assertDocs(
  files: string[],
  root = repository,
  versions?: Record<string, string>,
): Promise<void> {
  const current = versions ?? (await packageVersions(root));
  const problems: string[] = [];
  for (const path of files) {
    if (!/\.mdx?$/.test(path) || docsExcluded.test(path)) continue;
    const text = await readFile(resolve(root, path), "utf8");
    // Links inside code are examples, not references.
    const prose = text.replace(/^(```|~~~)[\s\S]*?^\1/gm, "").replace(/`[^`\n]*`/g, "");
    for (const [, target] of prose.matchAll(/\]\(([^)\s]+)(?:\s+"[^"]*")?\)/g)) {
      if (/^(?:[a-z][a-z0-9+.-]*:|#|\/)/i.test(target!)) continue;
      const file = target!.split(/[#?]/)[0]!;
      if (!file || (path.endsWith(".mdx") && file.endsWith("/"))) continue;
      if (!(await pathExists(resolve(root, dirname(path), file)))) problems.push(`${path}: broken link (${target})`);
    }
    for (const [, name, version] of text.matchAll(/(hitslop)@(\d+\.\d+\.\d+)\b/g))
      if (version !== current[name!])
        problems.push(`${path}: pins ${name}@${version}, but the tree is at ${current[name!]}`);
  }
  if (problems.length)
    throw new Error(`Docs are out of date:\n${[...new Set(problems)].map((item) => `  - ${item}`).join("\n")}`);
}

/** Frozen compatibility corpus entries (tests/compat) are permanent: against the
 * protected branch, a change may only add files to them. Git history is the authority;
 * hashes stored beside the files could be edited with them. CI names the base
 * (`HITSLOP_COMPAT_BASE`): the pull request's target, or the commit a push replaced. */
async function assertFrozenCorpus(): Promise<void> {
  const base = process.env.HITSLOP_COMPAT_BASE || "origin/master";
  const resolved = Bun.spawnSync(["git", "rev-parse", "--verify", "--quiet", `${base}^{commit}`], { cwd: repository });
  if (resolved.exitCode !== 0) {
    if (process.env.CI) throw new Error(`Cannot compare tests/compat with ${base}`);
    process.stdout.write(`! tests/compat not compared: ${base} is unavailable\n`);
    return;
  }
  const mergeBase = (await command(["git", "merge-base", "HEAD", base], repository, true)).trim();
  const listing = Bun.spawnSync(["git", "ls-tree", "--name-only", `${mergeBase}:tests/compat`], { cwd: repository, stderr: "ignore" });
  const entries = listing.exitCode === 0 ? listing.stdout.toString().split("\n").filter(Boolean) : [];
  const changed: string[] = [];
  for (const entry of entries) {
    const release = Bun.spawnSync(["git", "show", `${mergeBase}:tests/compat/${entry}/release.json`], { cwd: repository });
    if (release.exitCode !== 0 || JSON.parse(release.stdout.toString()).frozen !== true) continue;
    // Committed and uncommitted changes since the base; additions are the only allowed kind.
    const diff = await command(["git", "diff", "--name-status", "--no-renames", mergeBase, "--", `tests/compat/${entry}`], repository, true);
    changed.push(...diff.split("\n").filter((line) => line && !line.startsWith("A\t")));
  }
  if (changed.length)
    throw new Error(`Frozen compatibility corpus entries changed:\n${changed.map((line) => `  - ${line}`).join("\n")}`);
}

async function checkHygiene(): Promise<void> {
  const files = await gitFiles();
  await assertFrozenCorpus();
  for (const entry of await releases()) await verifyCorpus(entry.root, entry.release);
  assertTrackedHygiene(files);
  assertNoGeneratedSource(files);
  await assertPackageBoundaries(files);
  await assertTextHygiene(files);
  await assertDocs(files);
  await Promise.all([
    checkIgnored("AuthKey_FAKE123.p8", true),
    checkIgnored(".env.production", true),

    checkIgnored("_vibe/reference.png", true),
    checkIgnored("deferred/README.md", true),
    checkIgnored("archive/templates/unlisted-private/slop.ts", true),
    checkIgnored("archive/apple/example.swift", true),
    checkIgnored("archive/docs/review.md", true),
    checkIgnored("archive/spikes/boundary-simplification/README.md", true),
    checkIgnored("spikes/example/main.swift", true),
    checkIgnored("plans/example.md", true),
    checkIgnored("SLOPS.todo", true),
    checkIgnored("apps/promo/inspo/reference.mp4", true),
    checkIgnored("apps/promo/src/Promo.tsx", false),
    checkIgnored("docs/evidence/review.md", false),
    checkIgnored("examples/archive/example/slop.ts", true),
    assertSkill(".agents/skills/hitslop-authoring/SKILL.md"),
    assertSkill(".agents/skills/hitslop-design/SKILL.md"),
    assertSkill(".agents/skills/hitslop/SKILL.md"),
    assertSkill(".agents/skills/hitslop-document/SKILL.md"),
    assertSkill(".agents/skills/hitslop-native/SKILL.md"),
  ]);
  process.stdout.write("✓ repository hygiene and skills\n");
}

if (import.meta.main) await checkHygiene();
