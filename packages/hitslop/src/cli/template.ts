import { mkdir, mkdtemp, rm } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { execute, findDocumentEngine, type EngineOptions } from "./engine";
import { stageProject } from "./build";
import { defaultOutput, exists } from "./fs";


/** Native artwork comes from the installed app's renderer, never a compiler or checkout,
 * through the document engine. Returns the command that renders it. */
export async function prepareRenderer() {
  if (process.platform !== "darwin")
    throw new Error("--artwork native renders with hitSlop.app on macOS. Elsewhere, import preview and icon PNGs in defineSlop({ artwork }) to the project.");
  return { binary: await findDocumentEngine() };
}

/** Builds a project into a template file and returns its path: the stage, with artwork
 * the project supplies (`artwork/preview.png`, `artwork/icon.png`), packed by the file
 * engine. With a `render` command (from `prepareRenderer`), the app renders the artwork
 * the project does not supply, from a draft of the template. Without one, the build needs
 * no Mac. The engine publishes only a checked file and replaces only a template. */
export async function buildTemplate(source: string, render: EngineOptions | undefined, destination?: string | ((slug: string) => string)) {
  source = resolve(source);
  const temporary = await mkdtemp(join(tmpdir(), "hitslop-build-"));
  const stage = join(temporary, "stage");
  try {
    const app = await stageProject(source, stage);
    const slug = app.declaration.metadata.slug;
    const output = resolve(typeof destination === "function" ? destination(slug) : destination ?? defaultOutput(source, slug));
    if (!output.endsWith(".slop")) throw new Error("Build output must be a .slop file");
    const supplied = (name: string) => exists(join(stage, "artwork", name + ".png"), true);
    if (render && !((await supplied("preview")) && (await supplied("icon")))) {
      const draft = join(temporary, "draft.slop");
      await execute({ method: "pack", stage, app, file: draft });
      await mkdir(join(stage, "artwork"), { recursive: true });
      if (!(await supplied("preview")))
        await execute({ method: "screenshot", documentPath: draft, target: "preview", output: join(stage, "artwork/preview.png"), ifPresent: false }, render);
      if (!(await supplied("icon"))) {
        await execute({ method: "screenshot", documentPath: draft, target: "icon", output: join(stage, "artwork/icon.png"), ifPresent: true }, render);
        if (!(await supplied("icon"))) console.warn("No icon view; Finder will show the generic icon.");
      }
    }
    if (!render && !(await supplied("preview")))
      console.warn("No artwork: import preview and icon PNGs in defineSlop({ artwork }), or build with --artwork native on a Mac.");
    for (const role of ["preview", "icon"] as const)
      if (await supplied(role)) app.artwork[role] = `artwork/${role}.png`;
    await mkdir(dirname(output), { recursive: true });
    await execute({ method: "pack", stage, app, file: output });
    return output;
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
}
