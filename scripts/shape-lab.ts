/** Shape Lab source variants, native fixtures and a developer-only launcher. */
import { cp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { join, resolve } from "node:path";
import { buildProject } from "../packages/cli/src/build";
import { repository } from "./templates";

// These are manifest inputs to the production geometry parser, not a second renderer.
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
export const shapeLabRoot = join(repository, "generated/shape-lab");

export async function buildShapeLabVariant(kind: ShapeLabVariant, fallback = false) {
  const variant = shapeLabVariants[kind];
  const key = `${kind}${fallback ? "-fallback" : ""}`;
  const source = join(shapeLabRoot, "sources", key);
  await rm(source, { recursive: true, force: true });
  await cp(join(repository, "examples/slops/shape-lab"), source, { recursive: true });
  const manifest = JSON.parse(await readFile(join(source, "manifest.json"), "utf8"));
  await writeFile(
    join(source, "manifest.json"),
    JSON.stringify(
      {
        ...manifest,
        slug: `shape-lab-${key}`,
        title: `Shape Lab · ${variant.name}${fallback ? " · fallback" : ""}`,
        presentation: variant.presentation,
      },
      null,
      2,
    ) + "\n",
  );
  await writeFile(
    join(source, "variant.ts"),
    `export default ${JSON.stringify({ kind, name: variant.name, skin: kind === "washer", expectation: variant.expectation })};\n`,
  );
  // Generated source paths are deeper than the authored example.
  await writeFile(
    join(source, "tsconfig.json"),
    JSON.stringify({
      extends: join(repository, "tsconfig.json"),
      include: ["./**/*.svelte", "./**/*.ts"],
    }),
  );
  if (kind === "washer") {
    await mkdir(join(source, "assets"), { recursive: true });
    await cp(
      join(repository, "packages/cli/tests/fixtures/presentation/assets/washer.png"),
      join(source, "assets/washer.png"),
    );
  }
  if (fallback) await rm(join(source, "Export.svelte"));
  return buildProject(source, join(shapeLabRoot, key + ".slop"));
}

export async function buildShapeLabFixtures() {
  const paths: Record<string, string> = {};
  for (const kind of Object.keys(shapeLabVariants)) {
    for (const fallback of [false, true]) {
      paths[`shape-lab-${kind}${fallback ? "-fallback" : ""}`] = await buildShapeLabVariant(
        kind as ShapeLabVariant,
        fallback,
      );
    }
  }
  return paths;
}

if (import.meta.main) {
  const [command = "list", name = "rounded", mode] = process.argv.slice(2);
  if (command === "list") {
    for (const [key, value] of Object.entries(shapeLabVariants))
      console.log(`${key.padEnd(9)} ready — ${value.name}`);
  } else {
    if (
      !["build", "open"].includes(command) ||
      !Object.hasOwn(shapeLabVariants, name) ||
      (mode !== undefined && mode !== "--fallback")
    )
      throw new Error(
        "Usage: bun run shape:lab [list | build VARIANT | open VARIANT] [--fallback]",
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
      await mkdir(join(copy, ".."), { recursive: true });
      await cp(master, copy, { recursive: true });
      const child = Bun.spawn(["/usr/bin/open", "-a", app, copy], {
        stdout: "inherit",
        stderr: "inherit",
      });
      if (await child.exited) throw new Error("Could not open Shape Lab");
      console.log(`Editable copy: ${copy}`);
    }
  }
}
