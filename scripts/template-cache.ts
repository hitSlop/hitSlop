import { createHash } from "node:crypto";
import { cp, lstat, mkdir, readFile, readdir, rename, rm, writeFile } from "node:fs/promises";
import { dirname, join, relative } from "node:path";
import { parseManifest } from "../packages/schema/src/manifest";
import { localImports } from "../packages/cli/src/imports";
import { validateDocument, validateTheme } from "../packages/cli/src/core";
import { digest } from "./runtime-artifacts";

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
const hash = (value: string) => createHash("sha256").update(value).digest("hex");

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
      files[relative(root, path)] = createHash("sha256")
        .update(await readFile(path))
        .digest("hex");
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
  const child = Bun.spawn(command, { stdout: "pipe", stderr: "pipe" });
  const [out, error, code] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  if (code) throw new Error(`Cannot fingerprint ${command[0]}: ${error}`);
  return out.trim();
}

/** Files every template build reads: compiler, SDK, page shell and native renderer. */
export async function sharedTemplatePaths(repository: string, sources: string[]) {
  const native = "apps/apple/Packages/HitSlopApple";
  const paths = [
    "bun.lock",
    "packages/cli/package.json",
    "packages/cli/skills/hitslop-document",
    "tsconfig.json",
    "packages/document/src",
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
      "packages/cli/src/build-worker.ts",
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

/** Cheap package checks shared by cache reads and signed-app verification. */
export async function validateTemplate(path: string, slug: string) {
  const root = await lstat(path);
  if (!root.isDirectory() || root.isSymbolicLink())
    throw new Error(`Invalid template directory: ${slug}`);
  const allowed = new Set([
    "manifest.json",
    "assets",
    "state.schema.json",
    "initial.json",
    "QuickLook",
    ".agents",
  ]);
  for (const name of await readdir(path))
    if (!allowed.has(name)) throw new Error(`Unexpected template content: ${slug}/${name}`);
  const checksum = await digest(path); // Rejects symlinks and special files throughout the package.
  const manifest = parseManifest(JSON.parse(await readFile(join(path, "manifest.json"), "utf8")));
  if (manifest.slug !== slug) throw new Error(`Template slug mismatch: ${slug}`);
  await validateDocument(
    JSON.parse(await readFile(join(path, "state.schema.json"), "utf8")),
    JSON.parse(await readFile(join(path, "initial.json"), "utf8")),
  );
  await validateTheme(JSON.parse(await readFile(join(path, "assets/theme.json"), "utf8")));
  if (!(await readFile(join(path, "assets/app.js"))).length)
    throw new Error(`Empty template: ${slug}`);
  for (const name of ["Preview.png", "Icon.png"]) {
    const png = await readFile(join(path, "QuickLook", name)).catch((error) => {
      if (name === "Icon.png" && error.code === "ENOENT") return undefined;
      throw error;
    });
    if (!png) continue; // The CLI's --if-present icon capture deliberately permits no custom icon.
    if (
      png.length < 33 ||
      png.subarray(0, 8).toString("hex") !== "89504e470d0a1a0a" ||
      png.toString("ascii", 12, 16) !== "IHDR" ||
      !png.readUInt32BE(16) ||
      !png.readUInt32BE(20)
    )
      throw new Error(`Invalid template artwork: ${slug}/${name}`);
  }
  return checksum;
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
    const key = hash(JSON.stringify({ shared: this.shared, template }));
    const entry = join(this.directory, slug);
    let reason: string[];
    try {
      const metadata = JSON.parse(await readFile(join(entry, "entry.json"), "utf8"));
      if (metadata.key !== key)
        reason = [
          ...changedInputs(metadata.shared, this.shared).map((path) => `shared ${path}`),
          ...changedInputs(metadata.template, template).map((path) => `template ${path}`),
        ];
      else if (metadata.checksum !== (await validateTemplate(join(entry, "package.slop"), slug)))
        reason = ["cached package changed"];
      else {
        await cp(join(entry, "package.slop"), destination, {
          recursive: true,
          errorOnExist: true,
          force: false,
        });
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
      await cp(destination, join(stage, "package.slop"), { recursive: true });
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
