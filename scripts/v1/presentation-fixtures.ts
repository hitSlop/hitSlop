import { cp, readFile, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { buildProject } from "../../packages/cli/src/build";
import { repository } from "./templates";
export async function buildPresentationFixtures() {
  const parent = join(repository, "generated/v1/presentation");
  const fixture = join(repository, "packages/cli/tests/fixtures/presentation");
  const base = JSON.parse(await readFile(join(fixture, "manifest.json"), "utf8"));
  const packages: Record<string, string> = {};
  for (const kind of ["standard", "ellipse", "washer"]) {
    const source = join(parent, "sources", kind);
    // Replace, never merge: files removed from the fixture must not survive in the copy.
    await rm(source, { recursive: true, force: true });
    await cp(fixture, source, { recursive: true });
    await writeFile(
      join(source, "manifest.json"),
      JSON.stringify({
        ...base,
        slug: `presentation-${kind}`,
        title: `Presentation ${kind}`,
        presentation:
          kind === "washer"
            ? { width: 320, height: 320, skin: "assets/washer.png" }
            : {
                width: 320,
                height: 320,
                ...(kind === "ellipse" ? { shape: "ellipse", background: "transparent" } : {}),
              },
      }),
    );
    packages[kind] = await buildProject(source, join(parent, kind + ".slop"));
  }
  return packages;
}

if (import.meta.main) console.log(JSON.stringify(await buildPresentationFixtures(), null, 2));
