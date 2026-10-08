import { publishFolder, repository } from "../lib/artifacts";
import { chmod, copyFile, readdir } from "node:fs/promises";
import { join, resolve } from "node:path";
import { exists } from "../../packages/hitslop/src/cli/fs";
import { builtTemplates, type TemplateInventory } from "./discover";
import { validateTemplate } from "./cache";

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
  // The previous starters are read-only; replacing them needs write access first.
  if (await exists(destination)) await setWritable(destination, true);
  await publishFolder(destination, async (stage) => {
    for (const { slug, bundled } of inventory.templates) {
      if (!bundled) continue;
      const template = join(source, slug + ".slop");
      // A template file the engine opens, for this slug: never a document.
      await validateTemplate(template, slug);
      await copyFile(template, join(stage, slug + ".slop"));
    }
  });
  await setWritable(destination, false);
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
