import { cp, mkdir, realpath, writeFile, rm, rename } from "node:fs/promises";
import { basename, relative, resolve, join } from "node:path";
import { parseManifest, PackageFormat, RuntimeABI, SlopManifestSchema, type AppRow } from "@hitslop/schema";
import { validate } from "@hitslop/schema/validation";
import { validateDocument, validateTheme, validateWindowShape } from "./core";
import { exists } from "./fs";
import { localImports } from "./imports";
import { cliRoot } from "./paths";
import { start } from "./process";
import type { AppCompiler } from "./vite";
/** The runtime ABI comes from the project SDK, independently of the builder format. */
async function runtimeABI(source: string): Promise<number> {
  const sdk = await import(Bun.resolveSync("@hitslop/document/abi", source)).catch(() => {
    throw new Error("Install @hitslop/document in the project before building");
  });
  if (!Number.isInteger(sdk.RuntimeABI)) throw new Error("Update the project's @hitslop/document to build with this CLI");
  if (sdk.RuntimeABI < 1) throw new Error("Invalid SDK runtime ABI");
  // This CLI validates and previews only the runtimes it knows.
  if (sdk.RuntimeABI > RuntimeABI)
    throw new Error(`The project's @hitslop/document needs runtimeABI ${sdk.RuntimeABI}; this @hitslop/cli supports ${RuntimeABI}. Update @hitslop/cli`);
  return sdk.RuntimeABI;
}
/** The files a project's metadata comes from; dev also watches their local imports. */
export const metadataFiles = ["slop.ts", "schema.ts"] as const;
/** Runs the stage worker (`stage-worker.ts`) in a fresh process, which evaluates the
 * project's modules once; author logs pass through. */
export function stageWorker(args: string[], failure: string) {
  return start([process.execPath, join(cliRoot, "src/stage-worker.ts"), ...args], { cwd: cliRoot, inherit: true, failure });
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
export type LoadedProject = { slop: unknown; schema: unknown };
export async function loadProject(source: string): Promise<LoadedProject> {
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
  let manifest;
  try {
    manifest = parseManifest({ ...fields, slug: projectSlug(source) });
  } catch (error) {
    throw new Error(`slop.ts: ${error instanceof Error ? error.message : String(error)}`, { cause: error });
  }
  const runtime = await runtimeABI(source);
  if (!("skin" in manifest.presentation)) await validateWindowShape(manifest.presentation);
  await validateDocument(descriptor, initial);
  await validateTheme(theme);
  return {
    packageFormat: PackageFormat,
    runtimeABI: runtime,
    manifest,
    descriptor,
    initial,
    theme: theme as AppRow["theme"],
  };
}
/** Artwork a project supplies, packed as the file's preview and icon. */
const artwork = ["preview.png", "icon.png"];
/** Writes a build's stage at `stage`, replacing one there: `app.json` and, with
 * `compileApp`, the compiled app, its assets and the project's artwork. `slop-engine pack`
 * makes a `.slop` file of a stage. Without `compileApp`, `app.json` only: the dev server
 * serves the app from source. */
export async function stageProjectInBun(source: string, stage: string, compileApp?: AppCompiler) {
  source = resolve(source);
  stage = resolve(stage);
  const app = await normalizeApp(source, await loadProject(source));
  const ready = stage + ".building-" + crypto.randomUUID();
  await mkdir(join(ready, "assets"), { recursive: true });
  try {
    if (compileApp && (await exists(join(source, "assets"))))
      await cp(join(source, "assets"), join(ready, "assets"), { recursive: true });
    await compileApp?.(source, ready);
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
