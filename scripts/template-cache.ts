import { constants } from "node:fs";
import { copyFile, lstat, mkdir, readFile, readdir, rename, rm, writeFile } from "node:fs/promises";
import { join, relative } from "node:path";
import { localImports } from "../packages/cli/src/imports";
import { engine } from "../packages/cli/src/engine";
import { fileDigest, sha256 } from "./runtime-artifacts";
import { run } from "../packages/cli/src/process";

const ignored = new Set([
  "node_modules",
  "dist",
  ".git",
  ".build",
  ".swiftpm",
  ".DS_Store",
  ".svelte-kit",
  ".svelte-check",
  ".vite",
  ".crust",
]);


/** Named input hashes; retained in cache entries so misses can name their cause. */
export type Inputs = Record<string, string>;

/** Content and path based; timestamps and checkout locations never enter the key. */
export async function inputs(root: string, paths: string[]): Promise<Inputs> {
  const files: Inputs = {};
  async function visit(path: string, inputRoot = false) {
    const info = await lstat(path);
    if (info.isSymbolicLink() || (!info.isDirectory() && !info.isFile()))
      throw new Error(`Unsupported template input: ${path}`);
    if (info.isDirectory()) {
      files[relative(root, path) || "."] = "directory";
      for (const name of (await readdir(path)).sort())
        if (!inputRoot || !ignored.has(name)) await visit(join(path, name));
    } else
      files[relative(root, path)] = await fileDigest(path);
  }
  for (const path of [...paths].sort()) await visit(join(root, path), true);
  return files;
}

/** Relative imports reachable from the template compiler; CLI routing and help stay outside. */
async function compilerSources(repository: string, entries: string[]) {
  const files = await localImports(entries.map((entry) => join(repository, entry)));
  return files.map((file) => relative(repository, file));
}

async function version(command: string[]) {
  return (await run(command, { failure: `Cannot fingerprint ${command[0]}` })).trim();
}

/** Files every template build reads: compiler, SDK, page shell, file engine and native
 * renderer. */
export async function sharedTemplatePaths(repository: string, sources: string[]) {
  const native = "apps/apple/Packages/HitSlopApple";
  const paths = [
    "bun.lock",
    "Cargo.lock",
    "crates/hitslop-core/Cargo.toml",
    "crates/hitslop-core/src",
    "crates/slop-engine",
    "packages/cli/package.json",
    "packages/document/src",
    "packages/shell/src",
    "packages/shell/package.json",
    "packages/document/package.json",
    "packages/schema/src",
    "packages/schema/package.json",
    "packages/cli/shell",
    "scripts/build-templates.ts",
    "scripts/template-cache.ts",
    `${native}/Package.swift`,
    `${native}/Package.resolved`,
    ...["HitSlopCore", "HitSlopDocument", "HitSlopHost", "HitSlopNativeCLI"].map(
      (name) => `${native}/Sources/${name}`,
    ),
    ...(await compilerSources(repository, [
      "packages/cli/src/template.ts",
      "packages/cli/src/stage-worker.ts",
    ])),
  ];
  // Shared authoring configs and directories, but not other templates, docs or local tool state.
  for (const name of await readdir(join(repository, "examples/slops"))) {
    const path = join("examples/slops", name);
    if (
      !ignored.has(name) &&
      !name.startsWith(".") &&
      !name.endsWith(".md") &&
      !["archive", "bundled.json"].includes(name) &&
      !sources.includes(path)
    )
      paths.push(path);
  }
  return paths;
}

/** Shared files plus the toolchain that compiles and renders every template. */
export async function sharedTemplateInputs(repository: string, sources: string[]): Promise<Inputs> {
  const [build, xcode, swift] = await Promise.all([
    version(["/usr/bin/sw_vers", "-buildVersion"]),
    version(["xcodebuild", "-version"]),
    version(["swift", "--version"]),
  ]);
  return {
    ...(await inputs(repository, await sharedTemplatePaths(repository, sources))),
    "@macos": build,
    "@xcode": xcode,
    "@swift": swift,
    "@arch": process.arch,
    "@bun": Bun.version,
    "@debug": String(process.env.HITSLOP_DEBUG_BUILD === "1"),
  };
}

/** Name what differs between two input sets, for cache miss reports. */
export function changedInputs(previous: Inputs = {}, current: Inputs, limit = 5) {
  const changed = [...new Set([...Object.keys(previous), ...Object.keys(current)])]
    .filter((key) => previous[key] !== current[key])
    .sort();
  return changed.length > limit
    ? [...changed.slice(0, limit), `…and ${changed.length - limit} more`]
    : changed;
}

/** Checks shared by cache reads and signed-app verification: a regular template file the
 * engine opens, built for `slug`, with preview artwork (the icon is optional: a build
 * without an icon view has none). Returns the file's checksum. */
export async function validateTemplate(path: string, slug: string) {
  const info = await lstat(path);
  if (!info.isFile()) throw new Error(`Invalid template file: ${slug}`);
  const template = JSON.parse(await engine(["inspect", path]));
  if (template.kind !== "template") throw new Error(`Not a template: ${slug}`);
  if (template.manifest?.slug !== slug) throw new Error(`Template slug mismatch: ${slug}`);
  if (!template.artwork.some((artwork: { name: string }) => artwork.name === "preview"))
    throw new Error(`Template has no preview artwork: ${slug}`);
  return fileDigest(path);
}

/** Local and CI builds share this validated cache. A miss uses the ordinary builder. */
export class TemplateCache {
  /** Why each rebuilt template missed, by slug. */
  readonly misses = new Map<string, string[]>();

  constructor(
    readonly directory: string,
    readonly shared: Inputs,
  ) {}

  async build(source: string, slug: string, destination: string, build: () => Promise<unknown>) {
    if (!/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(slug)) throw new Error(`Invalid cache slug: ${slug}`);
    const template = await inputs(source, ["."]);
    const key = sha256(JSON.stringify({ shared: this.shared, template }));
    const entry = join(this.directory, slug);
    let reason: string[];
    try {
      const metadata = JSON.parse(await readFile(join(entry, "entry.json"), "utf8"));
      if (metadata.key !== key)
        reason = [
          ...changedInputs(metadata.shared, this.shared).map((path) => `shared ${path}`),
          ...changedInputs(metadata.template, template).map((path) => `template ${path}`),
        ];
      // The entry was validated when it was written: the same bytes are the same template.
      else if (metadata.checksum !== (await fileDigest(join(entry, "template.slop"))))
        reason = ["cached template changed"];
      else {
        await copyFile(join(entry, "template.slop"), destination, constants.COPYFILE_EXCL);
        return "hit" as const;
      }
    } catch (error) {
      // Missing or corrupt cache entries are disposable, never authoritative.
      reason = [
        (error as { code?: string }).code === "ENOENT"
          ? "no cache entry"
          : "unreadable cache entry",
      ];
    }
    this.misses.set(slug, reason);
    await build();
    const checksum = await validateTemplate(destination, slug);
    await mkdir(this.directory, { recursive: true });
    const stage = join(this.directory, `${slug}.building-${crypto.randomUUID()}`);
    try {
      await mkdir(stage);
      await copyFile(destination, join(stage, "template.slop"));
      await writeFile(
        join(stage, "entry.json"),
        JSON.stringify({ key, checksum, shared: this.shared, template }),
      );
      await rm(entry, { recursive: true, force: true });
      await rename(stage, entry);
    } finally {
      await rm(stage, { recursive: true, force: true });
    }
    return "built" as const;
  }

  async prune(slugs: string[]) {
    await mkdir(this.directory, { recursive: true });
    for (const entry of await readdir(this.directory, { withFileTypes: true })) {
      if (!entry.isDirectory() || slugs.includes(entry.name)) continue;
      const path = join(this.directory, entry.name);
      const metadata = await readFile(join(path, "entry.json"), "utf8").then(
        (text) => {
          try {
            return JSON.parse(text);
          } catch {
            return undefined;
          }
        },
        () => undefined,
      );
      // Only remove our own entries, even if a caller selects a shared directory.
      if (
        typeof metadata?.key === "string" &&
        typeof metadata.checksum === "string"
      )
        await rm(path, { recursive: true, force: true });
    }
  }
}
