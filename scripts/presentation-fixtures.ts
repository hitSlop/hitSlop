import { cp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { buildTemplate } from "../packages/cli/src/template";
import { repository } from "./templates";
import { buildShapeLabFixtures } from "./shape-lab";
export async function buildPresentationFixtures() {
  const parent = join(repository, "generated/presentation");
  const fixture = join(repository, "packages/cli/tests/fixtures/presentation");
  const packages: Record<string, string> = {};
  for (const kind of ["standard", "ellipse", "washer"]) {
    // The folder's name is the slug.
    const source = join(parent, "sources", `presentation-${kind}`);
    // Replace, never merge: files removed from the fixture must not survive in the copy.
    await rm(source, { recursive: true, force: true });
    await cp(fixture, source, { recursive: true });
    await writeFile(
      join(source, "variant.ts"),
      "export default " + JSON.stringify({
        title: `Presentation ${kind}`,
        presentation:
          kind === "washer"
            ? { width: 320, height: 320, skin: "assets/washer.png" }
            : {
                width: 320,
                height: 320,
                ...(kind === "ellipse" ? { shape: "50%", lockAspect: true, background: "transparent" } : {}),
              },
      }) + ";\n",
    );
    packages[kind] = await buildTemplate(source, undefined, join(parent, kind + ".slop"));
  }
  return { ...packages, ...await buildShapeLabFixtures() };
}

if (import.meta.main) console.log(JSON.stringify(await buildPresentationFixtures(), null, 2));
