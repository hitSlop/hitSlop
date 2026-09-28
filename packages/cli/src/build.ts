import { build as esbuild, type Plugin } from "esbuild";
import { sveltePlugin } from "./svelte-plugin";
import { cp, mkdir, readFile, writeFile, rm, rename, stat } from "node:fs/promises";
import { resolve, join, relative, isAbsolute } from "node:path";
import { parseManifest } from "@hitslop/schema";
import { fromDescriptor, validate } from "@hitslop/document";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";
import identity from "@hitslop/document/identity";
export const cliRoot = fileURLToPath(new URL("../", import.meta.url));
export const runtimeDirectory = join(cliRoot, "runtimes", String(identity.runtimeContract));
/**
 * App bundles contain authored code, Svelte and the app-side SDK (src/app plus pure
 * schema/theme helpers). The engine and host lifecycle ship with hitSlop; apps reach
 * them only through the ctx passed to mount.
 */
export const runtimePlugin: Plugin = {
  name: "host-runtime",
  setup(b) {
    b.onResolve({ filter: /^(loro-crdt|@hitslop\/document\/runtime)(\/.*)?$|^\/__runtime__\// }, (args) => {
      throw new Error(`App code cannot import ${args.path}; use ctx from the document SDK`);
    });
  },
};
const sdkSource = /(?:packages\/document|@hitslop\/document)\/src\/(.+)$/;
const appSDK = /^(app\/.+|schema\.ts|errors\.ts|theme\.ts)$/;
/** Reject engine code, host bridge access and remote resources needed to boot. */
export function checkAppBundle(inputs: string[], script: string, css: string) {
  for (const input of inputs) {
    const sdk = sdkSource.exec(input)?.[1];
    if (/loro-crdt/.test(input) || (sdk && !appSDK.test(sdk)))
      throw new Error(`Embedded runtime code rejected: ${input}`);
  }
  if (/messageHandlers|__slop\b/.test(script))
    throw new Error("App code cannot use the host bridge; use ctx from the document SDK");
  const remote = /@import\s+(?:url\()?\s*["']?https?:|@font-face\s*\{[^}]*url\(\s*["']?https?:/i.exec(css);
  if (remote || /\bimport\s*\(\s*["'`]https?:/.test(script))
    throw new Error("Apps must not need remote stylesheets, fonts or scripts to start; copy them into assets/");
}
export async function buildProject(source: string, destination?: string) {
  source = resolve(source);
  const output = destination
    ? resolve(destination)
    : join(
        source,
        "dist",
        parseManifest(JSON.parse(await readFile(join(source, "manifest.json"), "utf8"))).slug +
          ".slop",
      );
  const child = Bun.spawn(
    [process.execPath, join(cliRoot, "src/build-worker.ts"), source, output],
    { cwd: cliRoot, stdout: "inherit", stderr: "pipe" },
  );
  const [stderr, code] = await Promise.all([new Response(child.stderr).text(), child.exited]);
  if (code) throw new Error(stderr || "Authoring build failed");
  return output;
}
export async function buildProjectInBun(source: string, destination?: string) {
  source = resolve(source);
  const manifest = parseManifest(JSON.parse(await readFile(join(source, "manifest.json"), "utf8")));
  const out = destination ? resolve(destination) : join(source, "dist", manifest.slug + ".slop");
  if (out === source || !out.endsWith(".slop"))
    throw new Error("Build output must be a separate .slop directory");
  const installedIdentity = JSON.parse(
    await readFile(
      createRequire(join(source, "package.json")).resolve("@hitslop/document/identity"),
      "utf8",
    ),
  );
  if (Object.entries(identity).some(([key, value]) => installedIdentity[key] !== value))
    throw new Error(
      `Project SDK identity does not match the CLI's required SDK ${identity.sdkVersion}. Install @hitslop/document@${identity.sdkVersion} in the project.`,
    );
  const schemaModule = await import(join(source, "schema.ts"));
  const definition = Object.values(schemaModule).find((v: any) => v?.descriptor) as any;
  if (!definition) throw new Error("schema.ts must export a document definition");
  const descriptor = fromDescriptor(definition.descriptor).descriptor;
  const { default: initial } = await import(join(source, "initial.ts"));
  validate(descriptor.root, initial);
  const { default: theme } = await import(join(source, "theme.ts"));
  const stage = out + ".building-" + crypto.randomUUID();
  await mkdir(join(stage, "assets"), { recursive: true });
  if (
    await stat(join(source, "assets")).then(
      () => true,
      () => false,
    )
  )
    await cp(join(source, "assets"), join(stage, "assets"), { recursive: true });
  try {
    // App.svelte and styles.css need no entry file; main.ts may export another SlopApp.
    const custom = await stat(join(source, "main.ts")).then(() => true, () => false);
    const styles = await stat(join(source, "styles.css")).then(() => true, () => false);
    const result = await esbuild({
      ...(custom
        ? { entryPoints: [join(source, "main.ts")] }
        : {
            stdin: {
              contents:
                (styles ? 'import "./styles.css";\n' : "") +
                'import App from "./App.svelte";\nimport { defineSlop } from "@hitslop/document/svelte";\nexport default defineSlop(App);\n',
              resolveDir: source,
              sourcefile: "slop-entry.ts",
              loader: "ts",
            },
          }),
      bundle: true,
      outfile: join(stage, "assets/app.js"),
      format: "esm",
      platform: "browser",
      target: "safari17",
      loader: { ".ttf": "file", ".otf": "file", ".woff": "file", ".woff2": "file" },
      minify: process.env.HITSLOP_DEBUG_BUILD !== "1",
      conditions: ["browser", "default", "import", "svelte"],
      define: { "import.meta.env.DEV": "false", "import.meta.env.PROD": "true" },
      metafile: true,
      plugins: [
        runtimePlugin,
        {
          name: "copied-fonts",
          setup(b) {
            b.onResolve({ filter: /\.(ttf|otf|woff2?)([?#].*)?$/ }, async (args) => {
              if (args.pluginData?.resolvingFont) return;
              // Absolute browser URLs already refer to the copied assets directory.
              if (args.path.startsWith("/assets/")) return args.kind === "url-token"
                ? { path: args.path, external: true }
                : { path: args.path, namespace: "copied-font" };
              if (/^(https?:|data:)/.test(args.path)) return;
              const resolved = await b.resolve(args.path, {
                kind: args.kind, resolveDir: args.resolveDir,
                pluginData: { resolvingFont: true },
              });
              if (resolved.errors.length || resolved.external) return resolved;
              const path = relative(join(source, "assets"), resolved.path);
              if (isAbsolute(path) || path === ".." || path.startsWith("../")) return;
              const url = "/assets/" + path.split("/").map(encodeURIComponent).join("/") + resolved.suffix;
              if (args.kind === "url-token") return { path: url, external: true };
              return { path: url, namespace: "copied-font" };
            });
            b.onLoad({ filter: /.*/, namespace: "copied-font" }, ({ path }) => ({
              contents: `export default ${JSON.stringify(path)}`, loader: "js",
            }));
          },
        },
        sveltePlugin(cliRoot),
      ],
    });
    const css = join(stage, "assets/app.css");
    if (!(await stat(css).then(() => true, () => false))) await writeFile(css, "");
    checkAppBundle(
      Object.keys(result.metafile!.inputs),
      await readFile(join(stage, "assets/app.js"), "utf8"),
      await readFile(css, "utf8"),
    );
    await writeFile(join(stage, "manifest.json"), JSON.stringify(manifest, null, 2));
    await writeFile(join(stage, "state.schema.json"), JSON.stringify(descriptor, null, 2));
    await writeFile(join(stage, "initial.json"), JSON.stringify(initial, null, 2));
    await writeFile(join(stage, "assets/theme.json"), JSON.stringify(theme.defaults));
    await writeFile(
      join(stage, "assets/runtime.json"),
      JSON.stringify({
        runtimeContract: identity.runtimeContract,
        minRuntimeRevision: identity.runtimeRevision,
        sdkVersion: identity.sdkVersion,
        loroVersion: identity.loroVersion,
        protocolVersion: identity.protocolVersion,
      }),
    );
    await mkdir(join(stage, ".agents/skills/hitslop-document"), { recursive: true });
    await cp(
      join(cliRoot, "skills/hitslop-document/SKILL.md"),
      join(stage, ".agents/skills/hitslop-document/SKILL.md"),
    );
    if (
      await stat(out).then(
        () => true,
        () => false,
      )
    ) {
      // Build outputs are disposable only; never overwrite a writable document.
      if (
        await stat(join(out, "state")).then(
          () => true,
          () => false,
        )
      )
        throw new Error("Refusing to overwrite a writable document");
      await rm(out, { recursive: true });
    }
    await rename(stage, out);
    return out;
  } catch (error) {
    await rm(stage, { recursive: true, force: true });
    throw error;
  }
}
