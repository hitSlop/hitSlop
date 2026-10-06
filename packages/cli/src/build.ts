import { cp, mkdir, readFile, writeFile, rm, rename } from "node:fs/promises";
import { resolve, join } from "node:path";
import { parseManifest, PackageFormat, RuntimeABI } from "@hitslop/schema";
import { validateDocument, validateTheme, validateWindowShape } from "./core";
import { assertReplaceable, defaultOutput, exists } from "./fs";
import { cliRoot } from "./paths";
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
export async function buildProject(source: string, destination?: string) {
  source = resolve(source);
  const child = Bun.spawn(
    [process.execPath, join(cliRoot, "src/build-worker.ts"), source, ...(destination ? [resolve(destination)] : [])],
    { cwd: cliRoot, stdout: "inherit", stderr: "pipe" },
  );
  const [stderr, code] = await Promise.all([new Response(child.stderr).text(), child.exited]);
  if (code) throw new Error(stderr || "Authoring build failed");
  // The worker validated the manifest, so its slug names the default output.
  if (destination) return resolve(destination);
  return defaultOutput(source, JSON.parse(await readFile(join(source, "manifest.json"), "utf8")).slug);
}
/** Without `compileApp`, builds metadata only: the dev server serves the app and authored
 * assets from source, so neither is copied. */
export async function buildProjectInBun(
  source: string,
  destination: string | undefined,
  compileApp?: AppCompiler,
) {
  source = resolve(source);
  const manifest = parseManifest(JSON.parse(await readFile(join(source, "manifest.json"), "utf8")));
  const abi = await runtimeABI(source);
  if (!("skin" in manifest.presentation)) await validateWindowShape(manifest.presentation);
  const out = destination ? resolve(destination) : defaultOutput(source, manifest.slug);
  await assertReplaceable(out, source);
  // The app imports this default export as its live document.
  const { default: definition } = await import(join(source, "schema.ts"));
  if (!definition?.descriptor) throw new Error("schema.ts must default-export defineDocument(...)");
  const descriptor = definition.descriptor;
  const { default: initial } = await import(join(source, "initial.ts"));
  await validateDocument(descriptor, initial);
  const { default: theme } = await import(join(source, "theme.ts"));
  await validateTheme(theme.defaults);
  const stage = out + ".building-" + crypto.randomUUID();
  await mkdir(join(stage, "assets"), { recursive: true });
  if (compileApp && (await exists(join(source, "assets"))))
    await cp(join(source, "assets"), join(stage, "assets"), { recursive: true });
  try {
    await compileApp?.(source, stage);
    await writeFile(join(stage, "manifest.json"), JSON.stringify({ ...manifest, packageFormat: PackageFormat, runtimeABI: abi }, null, 2));
    await writeFile(join(stage, "state.schema.json"), JSON.stringify(descriptor, null, 2));
    await writeFile(join(stage, "initial.json"), JSON.stringify(initial, null, 2));
    await writeFile(join(stage, "assets/theme.json"), JSON.stringify(theme.defaults));
    if (compileApp) {
      await mkdir(join(stage, ".agents/skills/hitslop-document"), { recursive: true });
      await cp(
        join(cliRoot, "skills/hitslop-document/SKILL.md"),
        join(stage, ".agents/skills/hitslop-document/SKILL.md"),
      );
    }
    // Build outputs are disposable; `assertReplaceable` refused writable documents.
    if (await exists(out)) await rm(out, { recursive: true });
    await rename(stage, out);
    return out;
  } catch (error) {
    await rm(stage, { recursive: true, force: true });
    throw error;
  }
}
