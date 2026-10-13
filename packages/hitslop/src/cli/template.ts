import { mkdir, mkdtemp, rm } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { execute } from "./engine";
import { stageProject } from "./build";
import { defaultOutput } from "./fs";

/** Builds a project into a template file and returns its path: the stage, packed by the file
 * engine in one request. Artwork the project supplies (`defineSlop({ artwork })`) is stored as
 * given; unless `renderArtwork` is false, the engine also renders the Preview and Icon it does not
 * supply and stores them in the same pack. The engine publishes only a complete, checked file and
 * replaces only a template. */
export async function buildTemplate(
  source: string,
  destination?: string | ((slug: string) => string),
  { renderArtwork = true }: { renderArtwork?: boolean } = {},
) {
  source = resolve(source);
  const temporary = await mkdtemp(join(tmpdir(), "hitslop-build-"));
  const stage = join(temporary, "stage");
  try {
    const app = await stageProject(source, stage);
    const slug = app.declaration.metadata.slug;
    const output = resolve(
      typeof destination === "function"
        ? destination(slug)
        : (destination ?? defaultOutput(source, slug)),
    );
    if (!output.endsWith(".slop")) throw new Error("Build output must be a .slop file");
    await mkdir(dirname(output), { recursive: true });
    const { artwork } = await execute({ method: "pack", stage, app, file: output, renderArtwork });
    // A failed render fails the pack, so a role is absent only where nothing was asked to render it.
    const absent = (["preview", "icon"] as const).filter((role) => artwork[role] === "absent");
    if (absent.length)
      console.warn(
        `No ${absent.join(" or ")} artwork: import ${absent.join(" and ")} PNGs in defineSlop({ artwork }).`,
      );
    return output;
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
}
