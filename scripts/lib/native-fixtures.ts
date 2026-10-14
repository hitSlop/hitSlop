import { repository } from "./artifacts";
import { cp, mkdir, rm, writeFile } from "node:fs/promises";
import { basename, dirname, join } from "node:path";
import { stageProject } from "../../packages/hitslop/src/cli/build";
import { buildTemplate } from "../../packages/hitslop/src/cli/template";
import { templateCache } from "../templates/build";

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
    await cache.build(source, slug, destination, () => buildTemplate(source, destination, { renderArtwork: artwork }), artwork);
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

/** Builds the project `source` (whose folder name is its slug) into `destination`. */
type Builder = (source: string, slug: string, destination: string) => Promise<string>;

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

// These are presentation inputs to the production geometry parser, not a second renderer.
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

/** Every presentation fixture, by name: the standard, ellipse, glass, notch and washer controls. */
async function buildPresentationFixtures(build: Builder) {
  const parent = join(repository, "generated/presentation");
  const paths: Record<string, string> = {};
  for (const [kind, presentation] of Object.entries(presentationFixtures)) {
    const source = join(parent, "sources", `presentation-${kind}`);
    await withVariant(join(repository, "tests/presentation"), source, { title: `Presentation ${kind}`, presentation });
    paths[kind] = await build(source, `presentation-${kind}`, join(parent, kind + ".slop"));
  }
  return paths;
}
