import { cp, mkdir, mkdtemp, readFile, rename, rm, stat } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { builtTemplates, repository, type TemplateInventory } from "./templates";

async function permissions(path: string, mode: string) {
  const child = Bun.spawn(["/bin/chmod", "-R", mode, path], {
    stdout: "inherit",
    stderr: "inherit",
  });
  if (await child.exited) throw new Error(`Cannot set template permissions: ${path}`);
}

export async function embedTemplates(
  source: string,
  destination: string,
  inventory: TemplateInventory,
) {
  await mkdir(dirname(destination), { recursive: true });
  const stage = await mkdtemp(join(dirname(destination), ".starter-"));
  try {
    for (const { slug, bundled } of inventory.templates) {
      if (!bundled) continue;
      const template = join(source, slug + ".slop");
      const manifest = JSON.parse(await readFile(join(template, "manifest.json"), "utf8"));
      if (manifest.slug !== slug)
        throw new Error(`Invalid built template: ${slug}`);
      for (const name of ["state", "stores"])
        if (
          await stat(join(template, name)).then(
            () => true,
            (error) => {
              if (error.code === "ENOENT") return false;
              throw error;
            },
          )
        )
          throw new Error(`Mutable template: ${slug}`);
      await cp(template, join(stage, slug + ".slop"), { recursive: true });
    }
    if (
      await stat(destination).then(
        () => true,
        (error) => {
          if (error.code === "ENOENT") return false;
          throw error;
        },
      )
    ) {
      await permissions(destination, "u+w");
      await rm(destination, { recursive: true });
    }
    await rename(stage, destination);
    await permissions(destination, "a-w");
  } finally {
    await rm(stage, { recursive: true, force: true });
  }
}

if (import.meta.main) {
  const app = process.argv[2];
  if (!app) throw new Error("App bundle path required");
  await embedTemplates(
    join(repository, "generated/v1/templates"),
    join(resolve(app), "Contents/Resources/StarterTemplates"),
    await builtTemplates(),
  );
}
