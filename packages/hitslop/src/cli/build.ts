import { cp, mkdir, realpath, writeFile, rm, rename } from "node:fs/promises";
import { basename, relative, resolve, join } from "node:path";
import { PackageFormat, RuntimeABI, SlopManifestSchema, type AppRow } from "../schema/index";
import { validate } from "../schema/validation";
import { execute } from "./engine";
import { exists } from "./fs";
import { localImports } from "./imports";
import { cliRoot } from "./paths";
import { start, run } from "./process";
import type { AppCompiler } from "./vite";
/** The pinned package evaluates and builds against the runtime it ships. */
async function runtimeABI(source: string): Promise<number> {
  const { assertProjectPackage } = await import("./project");
  assertProjectPackage(source);
  return RuntimeABI;
}
/** The files a project's metadata comes from; dev also watches their local imports. */
export const metadataFiles = ["slop.ts", "schema.ts"] as const;
/** Runs the stage worker (`stage-worker.ts`) in a fresh process, which evaluates the
 * project's modules once; author logs pass through. */
export function stageWorker(args: string[], failure: string) {
  return start([process.execPath, join(cliRoot, "src/cli/stage-worker.ts"), ...args], { cwd: cliRoot, inherit: ["stdout"], failure });
}
/** A project's stage at `stage`, with its compiled app. */
export async function stageProject(source: string, stage: string) {
  await stageWorker([resolve(source), resolve(stage), "--compile"], "Authoring build failed").done;
}
/** Checks a project's `slop.ts` as a build does, writing nothing. */
export async function checkProject(source: string) {
  await stageWorker([resolve(source), "--check"], "slop.ts check failed").done;
}
/** A project's slug: the name of its folder. */
export function projectSlug(source: string): string {
  const slug = basename(resolve(source));
  try {
    validate(SlopManifestSchema.properties.slug, slug);
  } catch {
    throw new Error(`A project folder's name is its slug: rename "${slug}" to 2–64 lowercase letters or digits, separated by single hyphens`);
  }
  return slug;
}
/** A project's `slop.ts` and `schema.ts` default exports. Loading runs author code. */
type LoadedProject = { slop: unknown; schema: unknown };
export async function loadProject(source: string): Promise<LoadedProject> {
  (await import("./project")).assertProjectPackage(source);
  // slop.ts and its imports run in Bun when building: plain modules only, refused before
  // any of them runs.
  const entry = join(source, "slop.ts");
  if (await exists(entry, true)) {
    // Real paths on both sides, so a symlinked checkout is still inside itself.
    const root = await realpath(source);
    for (const path of await localImports([entry]).then((paths) => Promise.all(paths.map((path) => realpath(path)))).catch((error: Error) => {
      throw new Error(`slop.ts: ${error.message}`, { cause: error });
    })) {
      // A project is self-contained: its build reads nothing outside its folder.
      if (!path.startsWith(root + "/"))
        throw new Error(`slop.ts: ${relative(root, path)} is outside the project; keep what slop.ts imports inside its folder`);
      if (!/\.(?:[cm]?[jt]sx?|json)$/.test(path) || /\.svelte\.[jt]s$/.test(path))
        throw new Error(
          `slop.ts: ${relative(root, path)} cannot be imported here; slop.ts and the modules it imports run in Bun when building, so they import TypeScript, JavaScript and JSON, not Svelte or CSS`,
        );
    }
  }
  const load = async (file: string) => {
    try {
      return (await import(join(source, file))).default;
    } catch (error) {
      throw new Error(`${file}: ${error instanceof Error ? error.message : String(error)}`, { cause: error });
    }
  };
  return { slop: await load("slop.ts"), schema: await load("schema.ts") };
}
/** Checks a loaded project by the rules the file engine packs by, and returns its
 * `app.json`: the `app` row a `.slop` stores. */
export async function normalizeApp(source: string, { slop, schema }: LoadedProject): Promise<AppRow> {
  if (!slop || typeof slop !== "object") throw new Error("slop.ts must default-export defineSlop({ ... })");
  const descriptor = (schema as { descriptor?: unknown } | undefined)?.descriptor;
  if (!descriptor) throw new Error("schema.ts must default-export defineDocument(...)");
  const { schema: declared, initial, theme, ...fields } = slop as Record<string, unknown>;
  // The app imports schema.ts as its live document, so slop.ts must describe that one.
  if (declared !== schema) throw new Error("slop.ts: schema must be schema.ts's default export");
  if ("slug" in fields) throw new Error("slop.ts: remove slug; the project folder's name is the slug");
  for (const [key, value] of [["initial", initial], ["theme", theme]] as const)
    if (value === undefined) throw new Error(`slop.ts: ${key} is required`);
  const runtime = await runtimeABI(source);
  const app = {
    packageFormat: PackageFormat,
    runtimeABI: runtime,
    manifest: { ...fields, slug: projectSlug(source) },
    descriptor,
    initial,
    theme,
  };
  await execute({ method: "validateApp", app }).catch((error: Error) => { throw new Error(`slop.ts: ${error.message}`, { cause: error }); });
  return app as AppRow;
}
/** Artwork a project supplies, packed as the file's preview and icon. */
const artwork = ["preview.png", "icon.png"];
/** Writes a build's stage at `stage`, replacing one there: `app.json` and, with
 * `compileApp`, the compiled app, its assets and the project's artwork. The engine's `pack` JSON request
 * makes a `.slop` file of a stage. Without `compileApp`, `app.json` only: the dev server
 * serves the app from source. */
export async function stageProjectInBun(source: string, stage: string, compileApp?: AppCompiler) {
  source = resolve(source);
  stage = resolve(stage);
  const loaded = await loadProject(source);
  const app = await normalizeApp(source, loaded);
  const ready = stage + ".building-" + crypto.randomUUID();
  await mkdir(join(ready, "assets"), { recursive: true });
  try {
    if (compileApp && (await exists(join(source, "assets"))))
      await cp(join(source, "assets"), join(ready, "assets"), { recursive: true });
    if (await exists(join(source, "assets/__commands"))) throw new Error("assets/__commands is reserved for built command assets");
    await compileApp?.(source, ready);
    await (await import("./commands-build")).buildCommands(source, loaded.schema as object, compileApp ? ready : undefined);
    await writeFile(join(ready, "app.json"), JSON.stringify(app));
    for (const name of compileApp ? artwork : []) {
      if (!(await exists(join(source, "artwork", name), true))) continue;
      await mkdir(join(ready, "artwork"), { recursive: true });
      await cp(join(source, "artwork", name), join(ready, "artwork", name));
    }
    await rm(stage, { recursive: true, force: true });
    await rename(ready, stage);
  } catch (error) {
    await rm(ready, { recursive: true, force: true });
    throw error;
  }
}
