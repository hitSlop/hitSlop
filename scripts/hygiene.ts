import { releases } from "./compat";
import { verifyCorpus } from "./compat-integrity";
import { lstat, stat, realpath, readFile } from "node:fs/promises";
import { basename, dirname, extname, join, resolve } from "node:path";

const root = resolve(import.meta.dir, "..");
async function command(
  argv: string[],
  cwd = root,
  quiet = false,
  extraEnv: Record<string, string> = {},
): Promise<string> {
  const label = argv.join(" ");
  process.stdout.write(`→ ${label}\n`);
  const child = Bun.spawn(argv, {
    cwd,
    stdout: quiet ? "pipe" : "inherit",
    stderr: quiet ? "pipe" : "inherit",
    env: { ...process.env, ...extraEnv },
  });
  const [exitCode, stdout, stderr] = await Promise.all([
    child.exited,
    quiet ? new Response(child.stdout).text() : Promise.resolve(""),
    quiet ? new Response(child.stderr).text() : Promise.resolve(""),
  ]);
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
      root,
      true,
    )
  )
    .split("\0")
    .filter(Boolean);
  const existing = await Promise.all(
    candidates.map(async (path) => ((await pathExists(resolve(root, path))) ? path : null)),
  );
  return existing.filter((path): path is string => path !== null);
}

function assertTrackedHygiene(files: string[]): void {
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
    if (name === "Icon\r") failures.push(path);
    // Retain reviewed source and evidence referenced by the architecture plans.
    // Unlisted private archives remain excluded by .gitignore and this gate.
    if (
      /^(archive|examples\/archive|Prototypes)\//.test(path) &&
      !["archive/slops/", "archive/docs/", "archive/spikes/"].some(prefix => path.startsWith(prefix))
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
      path !== "packages/shell/src/boot.js" &&
      /^packages\/[^/]+\/src\//.test(path) &&
      /\.(?:d\.ts|js)$/.test(path),
  );
  if (generated.length)
    throw new Error(
      `Generated JavaScript/declarations found beside package source:\n${generated.map((path) => `  - ${path}`).join("\n")}`,
    );
}

async function checkIgnored(path: string, shouldIgnore: boolean): Promise<void> {
  const child = Bun.spawn(["git", "check-ignore", "--no-index", "-q", path], { cwd: root });
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
    if (!extensions.has(extname(path)) && ![".gitignore", "SLOPS.todo"].includes(basename(path)))
      continue;
    if (!(await stat(resolve(root, path))).isFile()) continue;
    const bytes = new Uint8Array(await Bun.file(resolve(root, path)).arrayBuffer());
    // Extensionless bundled executables are not release text.
    if (bytes.includes(0)) continue;
    const text = new TextDecoder().decode(bytes);
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
  root = resolve(import.meta.dir, ".."),
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
const packageNames = ["cli", "document", "schema"] as const;

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
  root = resolve(import.meta.dir, ".."),
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
    for (const [, name, version] of text.matchAll(/@hitslop\/(cli|document|schema)@(\d+\.\d+\.\d+)\b/g))
      if (version !== current[name!])
        problems.push(`${path}: pins @hitslop/${name}@${version}, but the tree is at ${current[name!]}`);
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
  const resolved = Bun.spawnSync(["git", "rev-parse", "--verify", "--quiet", `${base}^{commit}`], { cwd: root });
  if (resolved.exitCode !== 0) {
    if (process.env.CI) throw new Error(`Cannot compare tests/compat with ${base}`);
    process.stdout.write(`! tests/compat not compared: ${base} is unavailable\n`);
    return;
  }
  const mergeBase = (await command(["git", "merge-base", "HEAD", base], root, true)).trim();
  const listing = Bun.spawnSync(["git", "ls-tree", "--name-only", `${mergeBase}:tests/compat`], { cwd: root, stderr: "ignore" });
  const entries = listing.exitCode === 0 ? listing.stdout.toString().split("\n").filter(Boolean) : [];
  const changed: string[] = [];
  for (const entry of entries) {
    const release = Bun.spawnSync(["git", "show", `${mergeBase}:tests/compat/${entry}/release.json`], { cwd: root });
    if (release.exitCode !== 0 || JSON.parse(release.stdout.toString()).frozen !== true) continue;
    // Committed and uncommitted changes since the base; additions are the only allowed kind.
    const diff = await command(["git", "diff", "--name-status", "--no-renames", mergeBase, "--", `tests/compat/${entry}`], root, true);
    changed.push(...diff.split("\n").filter((line) => line && !line.startsWith("A\t")));
  }
  if (changed.length)
    throw new Error(`Frozen compatibility corpus entries changed:\n${changed.map((line) => `  - ${line}`).join("\n")}`);
}

export async function checkHygiene(): Promise<void> {
  const files = await gitFiles();
  await assertFrozenCorpus();
  for (const entry of await releases()) await verifyCorpus(entry.root, entry.release);
  assertTrackedHygiene(files);
  assertNoGeneratedSource(files);
  await assertTextHygiene(files);
  await assertDocs(files);
  await Promise.all([
    checkIgnored("AuthKey_FAKE123.p8", true),
    checkIgnored(".env.production", true),

    checkIgnored("_vibe/reference.png", true),
    checkIgnored("deferred/README.md", true),
    checkIgnored("archive/templates/unlisted-private/manifest.json", true),
    checkIgnored("archive/apple/example.swift", true),
    checkIgnored("examples/archive/example/manifest.json", true),
    assertSkill(".agents/skills/hitslop-authoring/SKILL.md"),
    assertSkill(".agents/skills/hitslop-design/SKILL.md"),
    assertSkill(".agents/skills/hitslop/SKILL.md"),
    assertSkill(".agents/skills/hitslop-document/SKILL.md"),
    assertSkill(".agents/skills/hitslop-native/SKILL.md"),
  ]);
  process.stdout.write("✓ repository hygiene and skills\n");
}

if (import.meta.main) await checkHygiene();
