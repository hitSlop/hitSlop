/** Window presentation fixtures for native tests (the presentation controls and the Shape
 * Lab variants) and a developer-only Shape Lab launcher. */
import { repository } from "./runtime-artifacts";
import { cp, mkdir, rm, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { buildTemplate } from "../packages/cli/src/template";
import { run } from "../packages/cli/src/process";
import { createDocument } from "./helper";

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
  await writeFile(join(source, "variant.ts"), `export default ${JSON.stringify(value)};\n`);
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
  if (fallback) await rm(join(source, "Export.svelte"));
  return build(source, `shape-lab-${key}`, join(shapeLabRoot, key + ".slop"));
}

/** Every presentation fixture, by name: the standard, ellipse and washer controls, and each
 * Shape Lab variant with a dedicated and a fallback export. */
export async function buildPresentationFixtures(build = direct) {
  const parent = join(repository, "generated/presentation");
  const paths: Record<string, string> = {};
  for (const kind of ["standard", "ellipse", "washer"]) {
    const source = join(parent, "sources", `presentation-${kind}`);
    await withVariant(join(repository, "tests/presentation"), source, {
      title: `Presentation ${kind}`,
      presentation:
        kind === "washer"
          ? { width: 320, height: 320, skin: "assets/washer.png" }
          : { width: 320, height: 320, ...(kind === "ellipse" ? { shape: "50%", lockAspect: true, background: "transparent" } : {}) },
    });
    paths[kind] = await build(source, `presentation-${kind}`, join(parent, kind + ".slop"));
  }
  for (const kind of Object.keys(shapeLabVariants) as ShapeLabVariant[])
    for (const fallback of [false, true])
      paths[`shape-lab-${kind}${fallback ? "-fallback" : ""}`] = await buildShapeLabVariant(kind, fallback, build);
  return paths;
}

if (import.meta.main) {
  const [command = "list", name = "rounded", mode] = process.argv.slice(2);
  if (command === "list") {
    for (const [key, value] of Object.entries(shapeLabVariants))
      console.log(`${key.padEnd(9)} ready — ${value.name}`);
  } else if (command === "fixtures") {
    console.log(JSON.stringify(await buildPresentationFixtures(), null, 2));
  } else {
    if (
      !["build", "open"].includes(command) ||
      !Object.hasOwn(shapeLabVariants, name) ||
      (mode !== undefined && mode !== "--fallback")
    )
      throw new Error(
        "Usage: bun run shape:lab [list | fixtures | build VARIANT | open VARIANT] [--fallback]",
      );
    const master = await buildShapeLabVariant(name as ShapeLabVariant, mode === "--fallback");
    console.log(master);
    if (command === "open") {
      const app = resolve(
        process.env.HITSLOP_APP ?? join(repository, "generated/app/hitSlop.app"),
      );
      if (!existsSync(app))
        throw new Error("Build the matching app with bun run apple:build, or set HITSLOP_APP.");
      const copy = join(
        repository,
        ".hitslop/shape-lab/documents",
        `${name}-${crypto.randomUUID()}.slop`,
      );
      await mkdir(dirname(copy), { recursive: true });
      // A copy of a template is a template; the app's helper creates a document from it.
      await createDocument(master, copy, { helper: join(app, "Contents/Helpers/hitslop-native") });
      await run(["/usr/bin/open", "-a", app, copy], { failure: "Could not open Shape Lab" });
      console.log(`Editable copy: ${copy}`);
    }
  }
}
