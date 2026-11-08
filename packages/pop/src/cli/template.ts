import { mkdir, mkdtemp, rm } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { execute } from "./engine";
import { stageProject } from "./build";
import { defaultOutput } from "./fs";
import { SharingHost } from "../wire/constants.generated";

/** Live collaboration first uploads the embedded app and saved history as one seed of at most
 * `SharingHost.seedBytes`. An app storing three quarters of that leaves about 4 MiB for history
 * and SQLite overhead, so builds warn there. Attachments travel separately and do not count. */
const largeAppBytes = (SharingHost.seedBytes / 4) * 3;

/** Builds a project into a template file and returns its path: the stage, packed by the file
 * engine in one request. Artwork the project supplies (`definePop({ artwork })`) is stored as
 * given; unless `renderViews` is false, the engine checks every declared view (including PNG
 * and PDF Export) and stores only Preview/Icon artwork the project does not supply. The engine
 * publishes only a complete, checked file and replaces only a template. */
export async function buildTemplate(
  source: string,
  destination?: string | ((slug: string) => string),
  { renderViews = true }: { renderViews?: boolean } = {},
) {
  source = resolve(source);
  const temporary = await mkdtemp(join(tmpdir(), "pop-build-"));
  const stage = join(temporary, "stage");
  try {
    const app = await stageProject(source, stage);
    const slug = app.declaration.metadata.slug;
    const output = resolve(
      typeof destination === "function"
        ? destination(slug)
        : (destination ?? defaultOutput(source, slug)),
    );
    if (!output.endsWith(".pop")) throw new Error("Build output must be a .pop file");
    await mkdir(dirname(output), { recursive: true });
    const { artwork } = await execute({ method: "pack", stage, app, file: output, renderViews });
    // A failed render fails the pack, so a role is absent only where nothing was asked to render it.
    const absent = (["preview", "icon"] as const).filter((role) => artwork[role] === "absent");
    if (absent.length)
      console.warn(
        `No ${absent.join(" or ")} artwork: import ${absent.join(" and ")} PNGs in definePop({ artwork }).`,
      );
    const { info } = await execute({ method: "inspect", file: output });
    if (info.storedAssetBytes > largeAppBytes)
      console.warn(
        `The app stores ${(info.storedAssetBytes / 2 ** 20).toFixed(1)} MiB of resources. Live collaboration cannot share a document whose app and saved history exceed ${SharingHost.seedBytes / 2 ** 20} MiB: keep large media in document attachments.`,
      );
    return output;
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
}
