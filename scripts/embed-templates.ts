import { chmod, copyFile, mkdir, mkdtemp, readdir, rename, rm, stat } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { builtTemplates, repository, type TemplateInventory } from "./templates";
import { validateTemplate } from "./template-cache";

/** Starter templates ship read-only: the folder and each template file in it. */
async function setWritable(folder: string, writable: boolean) {
  if (writable) await chmod(folder, 0o755);
  for (const name of await readdir(folder)) await chmod(join(folder, name), writable ? 0o644 : 0o444);
  if (!writable) await chmod(folder, 0o555);
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
      // A template file the engine opens, for this slug: never a document.
      await validateTemplate(template, slug);
      await copyFile(template, join(stage, slug + ".slop"));
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
      await setWritable(destination, true);
      await rm(destination, { recursive: true });
    }
    await rename(stage, destination);
    await setWritable(destination, false);
  } finally {
    await rm(stage, { recursive: true, force: true });
  }
}

if (import.meta.main) {
  const app = process.argv[2];
  if (!app) throw new Error("App bundle path required");
  await embedTemplates(
    join(repository, "generated/templates"),
    join(resolve(app), "Contents/Resources/StarterTemplates"),
    await builtTemplates(),
  );
}
