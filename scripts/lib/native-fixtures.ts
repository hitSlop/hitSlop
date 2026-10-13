import { repository } from "./artifacts";
import { cp, mkdir, rm, writeFile } from "node:fs/promises";
import { basename, dirname, join } from "node:path";
import { stageProject } from "../../packages/hitslop/src/cli/build";
import { buildTemplate } from "../../packages/hitslop/src/cli/template";
import { templateCache } from "../templates/build";
import { debugHelper } from "./native";
import { Database } from "bun:sqlite";
import { bytesOf, resourceKey, typeOf } from "../../packages/hitslop/src/cli/definition-resources";

export const nativeFixtureSlugs = ["document-fixture"];
const output = join(repository, "generated/native-fixtures");

/** Native tests' templates: the minimal document fixture, the ABI owner app and the
 * presentation fixtures, by name. All go through the template cache, so a run that changed
 * none of their inputs only copies them. */
export async function prepareNativeFixtures() {
  const cache = await templateCache("fixtures", false);
  // Only the document fixture needs initial artwork for catalog and save/close tests.
  const build: Builder = async (source, slug, destination) => {
    await mkdir(dirname(destination), { recursive: true });
    await rm(destination, { force: true });
    const artwork = slug === "document-fixture";
    const renderer = artwork ? { env: { ...process.env, HITSLOP_NATIVE_CLI: debugHelper } } : undefined;
    await cache.build(source, slug, destination, () => buildTemplate(source, renderer, destination), artwork);
    return destination;
  };
  await build(join(repository, "tests/apps/document"), "document-fixture", join(output, "document-fixture.slop"));
  await build(join(repository, "tests/abi/owner-svelte"), "owner-svelte", join(repository, "generated/abi/owner-svelte.slop"));
  return buildPresentationFixtures(build);
}

/** The document fixture's build stage beside it (`generated/native-fixtures/<slug>`), which
 * benchmarks change before packing. */
export async function stageNativeFixtures() {
  for (const slug of nativeFixtureSlugs) {
    const stage = join(output, slug);
    await rm(stage, { recursive: true, force: true });
    const input = await stageProject(join(repository, "tests/apps/document"), stage);
    await writeFile(join(stage, "input.json"), JSON.stringify(input));
  }
}

// These are presentation inputs to the production geometry parser, not a second renderer.
export const shapeLabVariants = {
  rounded: {
    name: "Rounded baseline",
    presentation: { width: 480, height: 360 },
    expectation: "Corners clip; the centre receives input.",
  },
  radii: {
    name: "Unequal elliptical corners",
    presentation: { width: 480, height: 360, shape: "48px 8px 72px 0 / 24px 32px 18px 0" },
    expectation: "Four different corners; orientation must match the labels.",
  },
  concave: {
    name: "Concave silhouette",
    presentation: {
      width: 480,
      height: 360,
      shape: {
        path: "M24 0 H456 Q480 0 480 24 V132 L432 156 L480 180 V336 Q480 360 456 360 H0 V48 Z",
        viewBox: [480, 360],
      },
    },
    expectation: "The right-hand notch sends clicks to the receiver.",
  },
  hole: {
    name: "Offset hole / free resize",
    presentation: {
      width: 480,
      height: 360,
      shape: {
        path: "M24 0 H456 Q480 0 480 24 V336 Q480 360 456 360 H0 V48 Z M350 56 a44 32 0 1 0 0 64 a44 32 0 1 0 0 -64 Z",
        viewBox: [480, 360],
        fillRule: "evenodd",
      },
    },
    expectation: "The upper-right hole sends clicks to the receiver; proportions can change.",
  },
  locked: {
    name: "Offset hole / locked aspect",
    presentation: {
      width: 480,
      height: 360,
      lockAspect: true,
      shape: {
        path: "M24 0 H456 Q480 0 480 24 V336 Q480 360 456 360 H0 V48 Z M350 56 a44 32 0 1 0 0 64 a44 32 0 1 0 0 -64 Z",
        viewBox: [480, 360],
        fillRule: "evenodd",
      },
    },
    expectation: "The hole stays proportional; the window remains 4:3.",
  },
  washer: {
    name: "PNG washer control",
    presentation: { width: 320, height: 320, skin: "assets/washer.png" },
    expectation: "The centre receives no input; this PNG skin cannot resize.",
  },
} as const;
export type ShapeLabVariant = keyof typeof shapeLabVariants;
const shapeLabRoot = join(repository, "generated/shape-lab");
/** Builds the project `source` (whose folder name is its slug) into `destination`. */
export type Builder = (source: string, slug: string, destination: string) => Promise<string>;
const direct: Builder = (source, _, destination) => buildTemplate(source, undefined, destination);

/** A copy of `fixture` at `source` with this `variant.ts`: the fixtures vary only there. */
async function withVariant(fixture: string, source: string, value: unknown) {
  // Replace, never merge: files removed from the fixture must not survive in the copy.
  await rm(source, { recursive: true, force: true });
  await cp(fixture, source, { recursive: true });
  const { presentation, ...fields } = value as any;
  const {skin, ...window} = presentation;
  const declaration = {...fields,slug:basename(source),window:{kind:skin ? "skin" : "standard",...window}};
  const code = skin
    ? `import image from ${JSON.stringify("./" + skin)};\nconst variant = ${JSON.stringify(declaration)} as const;\nexport default {...variant,window:{...variant.window,image}};\n`
    : `export default ${JSON.stringify(declaration)} as const;\n`;
  await writeFile(join(source, "variant.ts"), code);
}

export async function buildShapeLabVariant(kind: ShapeLabVariant, fallback = false, build = direct) {
  const variant = shapeLabVariants[kind];
  const key = `${kind}${fallback ? "-fallback" : ""}`;
  // The folder's name is the slug.
  const source = join(shapeLabRoot, "sources", `shape-lab-${key}`);
  await withVariant(join(repository, "tests/apps/shape-lab"), source, {
    kind,
    name: variant.name,
    title: `Shape Lab · ${variant.name}${fallback ? " · fallback" : ""}`,
    presentation: variant.presentation,
    skin: kind === "washer",
    expectation: variant.expectation,
  });
  if (kind === "washer") {
    await mkdir(join(source, "assets"), { recursive: true });
    await cp(join(repository, "tests/presentation/assets/washer.png"), join(source, "assets/washer.png"));
  }
  const file = await build(source, `shape-lab-${key}`, join(shapeLabRoot, key + ".slop"));
  if (fallback) await makeShapeLabFallback(source, file);
  return file;
}

/** A disposable stored-app fixture for the explicit WebKit provider. Authoring requires
 * Preview.svelte: first build/cache a valid package, then replace its browser
 * entry with one that actually omits Preview and Export. Flipping metadata alone would leave the
 * dedicated component registered by svelteApp and would not exercise App fallback. */
async function makeShapeLabFallback(source: string, file: string) {
  // CI imports fixture helpers while selecting tiers, before dependencies are installed.
  // Load the browser compiler only when this fixture is actually built.
  const [{ build: viteBuild }, { appConfig }, { discoverRoles, uiEntrySource }] = await Promise.all([
    import("vite"),
    import("../../packages/hitslop/src/cli/vite"),
    import("../../packages/hitslop/src/cli/app-vite"),
  ]);
  const entry = "\0hitslop-shape-lab-fallback";
  const roles = await discoverRoles(source);
  const config = await appConfig(source);
  const result = await viteBuild({
    ...config,
    base: "/assets/",
    plugins: [{
      name: "shape-lab-fallback-fixture",
      resolveId(id) { if (id === entry) return entry; },
      load(id) { if (id === entry) return uiEntrySource({ ...roles, preview: undefined, export: undefined }); },
    }, ...(config.plugins ?? [])],
    build: {
      write: false, target: "safari17", minify: "oxc", assetsInlineLimit: 0,
      cssCodeSplit: false, modulePreload: false,
      rolldownOptions: {
        input: entry, preserveEntrySignatures: "strict",
        output: {
          format: "es", codeSplitting: false, entryFileNames: "ui.js",
          assetFileNames(asset) {
            return asset.names.some(name => name.endsWith(".css"))
              ? "ui.css" : resourceKey(asset.source, asset.names[0] ?? "asset.bin");
          },
        },
      },
    },
  });
  if ("on" in result) throw new Error("Shape Lab fixtures cannot use a watch build");
  const outputs = (Array.isArray(result) ? result : [result]).flatMap(output => output.output);
  if (!outputs.some(output => output.type === "chunk" && output.fileName === "ui.js"))
    throw new Error("Shape Lab fallback did not emit its browser entry");

  // Like Swift Fixtures.sql, only test fixture construction bypasses the app seal.
  // The template cache retains the valid package; this generated copy is disposable.
  const database = new Database(file);
  try {
    const triggers = database.query("SELECT name,sql FROM sqlite_master WHERE type='trigger'").all() as { name: string; sql: string }[];
    const quote = (name: string) => `"${name.replaceAll('"', '""')}"`;
    database.transaction(() => {
      for (const trigger of triggers) database.exec(`DROP TRIGGER ${quote(trigger.name)}`);
      const app = database.query("SELECT definition_json FROM app WHERE id=1").get() as { definition_json: string };
      const definition = JSON.parse(app.definition_json);
      definition.views.export = false;
      definition.views.preview = false;
      database.query("UPDATE app SET definition_json=? WHERE id=1").run(JSON.stringify(definition));
      database.query("DELETE FROM assets WHERE key IN ('export.js', 'preview.js')").run();
      const put = database.query("INSERT OR REPLACE INTO assets(key,media_type,encoding,size,bytes) VALUES(?,?,'identity',?,?)");
      for (const output of outputs) {
        const bytes = bytesOf(output.type === "chunk" ? output.code : output.source);
        put.run(output.fileName, typeOf(output.fileName)[0]!, bytes.length, bytes);
      }
      for (const trigger of triggers) database.exec(trigger.sql);
    })();
  } finally { database.close(); }
}

const presentationFixtures = {
  standard: { width: 320, height: 320 },
  ellipse: { width: 320, height: 320, shape: "50%", lockAspect: true, background: "transparent" },
  glass: { width: 320, height: 320, background: "glass" },
  notch: {
    width: 320,
    height: 320,
    shape: { path: "M16 0 H304 Q320 0 320 16 V120 L280 160 L320 200 V304 Q320 320 304 320 H0 V32 Z", viewBox: [320, 320] },
  },
  washer: { width: 320, height: 320, skin: "assets/washer.png" },
  "washer-2x": { width: 320, height: 320, skin: "assets/washer@2x.png" },
};

/** Every presentation fixture, by name: the standard, ellipse, glass, notch and washer controls, and
 * each Shape Lab variant with a dedicated and a fallback export. */
export async function buildPresentationFixtures(build = direct) {
  const parent = join(repository, "generated/presentation");
  const paths: Record<string, string> = {};
  for (const [kind, presentation] of Object.entries(presentationFixtures)) {
    const source = join(parent, "sources", `presentation-${kind}`);
    await withVariant(join(repository, "tests/presentation"), source, { title: `Presentation ${kind}`, presentation });
    paths[kind] = await build(source, `presentation-${kind}`, join(parent, kind + ".slop"));
  }
  for (const kind of Object.keys(shapeLabVariants) as ShapeLabVariant[])
    for (const fallback of [false, true])
      paths[`shape-lab-${kind}${fallback ? "-fallback" : ""}`] = await buildShapeLabVariant(kind, fallback, build);
  return paths;
}
