import { repository } from "./artifacts";
import { cp, mkdir, rm, writeFile } from "node:fs/promises";
import { basename, dirname, join } from "node:path";
import { stageProject } from "../../packages/hitslop/src/cli/build";
import { buildTemplate } from "../../packages/hitslop/src/cli/template";
import { buildTemplates, templateCache } from "../templates/build";
import { discoverTemplates } from "../templates/discover";

export const nativeFixtureSlugs = ["quick-checklist"];
const output = join(repository, "generated/native-fixtures");

/** Native tests' templates: the active trial template, the ABI owner app and the
 * presentation fixtures, by name. All go through the template cache, so a run that changed
 * none of their inputs only copies them. */
export async function prepareNativeFixtures() {
  await buildTemplates(output, nativeFixtureSlugs);
  const cache = await templateCache("fixtures");
  // Fixtures render no native artwork.
  const build: Builder = async (source, slug, destination) => {
    await mkdir(dirname(destination), { recursive: true });
    await rm(destination, { force: true });
    await cache.build(source, slug, destination, () => buildTemplate(source, undefined, destination), false);
    return destination;
  };
  await build(join(repository, "tests/abi/owner-svelte"), "owner-svelte", join(repository, "generated/abi/owner-svelte.slop"));
  return buildPresentationFixtures(build);
}

/** Each trial template's build stage beside it (`generated/native-fixtures/<slug>`), which
 * benchmarks change before packing. */
export async function stageNativeFixtures() {
  const templates = await discoverTemplates();
  for (const slug of nativeFixtureSlugs) {
    const stage = join(output, slug);
    await rm(stage, { recursive: true, force: true });
    const input = await stageProject(templates.find((template) => template.slug === slug)!.source, stage);
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
  await withVariant(join(repository, "examples/slops/shape-lab"), source, {
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
  if (fallback) {
    const declaration = join(source, "slop.ts");
    const text = await Bun.file(declaration).text();
    await writeFile(declaration, text.replace('import Export from "./Export.svelte";\n', '').replace('  export: Export,\n', ''));
    await rm(join(source, "Export.svelte"));
  }
  return build(source, `shape-lab-${key}`, join(shapeLabRoot, key + ".slop"));
}

const presentationFixtures = {
  standard: { width: 320, height: 320 },
  ellipse: { width: 320, height: 320, shape: "50%", lockAspect: true, background: "transparent" },
  glass: { width: 320, height: 320, background: "glass" },
  washer: { width: 320, height: 320, skin: "assets/washer.png" },
};

/** Every presentation fixture, by name: the standard, ellipse, glass and washer controls, and
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
