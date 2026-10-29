import { mkdir, mkdtemp, rm } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { execute } from "./engine";
import { stageProject } from "./build";
import { defaultOutput } from "./fs";

/** Builds a project into a template file and returns its path: the stage, packed by the file
 * engine in one request. Artwork the project supplies (`defineTiny({ artwork })`) is stored as
 * given; unless `renderViews` is false, the engine checks every declared view (including PNG
 * and PDF Export) and stores only Preview/Icon artwork the project does not supply. The engine
 * publishes only a complete, checked file and replaces only a template. */
export async function buildTemplate(
  source: string,
  destination?: string | ((slug: string) => string),
  { renderViews = true }: { renderViews?: boolean } = {},
) {
  source = resolve(source);
  const temporary = await mkdtemp(join(tmpdir(), "tinys-build-"));
  const stage = join(temporary, "stage");
  try {
    const app = await stageProject(source, stage);
    const slug = app.declaration.metadata.slug;
    const output = resolve(
      typeof destination === "function"
        ? destination(slug)
        : (destination ?? defaultOutput(source, slug)),
    );
    if (!output.endsWith(".tiny")) throw new Error("Build output must be a .tiny file");
    await mkdir(dirname(output), { recursive: true });
    const { artwork } = await execute({ method: "pack", stage, app, file: output, renderViews });
    // A failed render fails the pack, so a role is absent only where nothing was asked to render it.
    const absent = (["preview", "icon"] as const).filter((role) => artwork[role] === "absent");
    if (absent.length)
      console.warn(
        `No ${absent.join(" or ")} artwork: import ${absent.join(" and ")} PNGs in defineTiny({ artwork }).`,
      );
    return output;
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
}
