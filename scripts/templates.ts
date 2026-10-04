import { readdir, readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { exists } from "../packages/cli/src/fs";
import { projectSlug } from "../packages/cli/src/build";

export const repository = resolve(import.meta.dir, "..");
export type TemplateSource = { slug: string; source: string; bundled: boolean };

/** Only immediate authored projects (folders with a `slop.ts`) are active; archives and
 * build outputs are not scanned. Discovery never runs author code: a folder's name is its
 * slug. */
export async function discoverTemplates(
  root = join(repository, "examples/slops"),
): Promise<TemplateSource[]> {
  const selected: unknown = JSON.parse(await readFile(join(root, "bundled.json"), "utf8"));
  if (
    !Array.isArray(selected) ||
    selected.some((value) => typeof value !== "string") ||
    new Set(selected).size !== selected.length
  )
    throw new Error("bundled.json must be an array of unique template slugs");
  const templates: TemplateSource[] = [];
  const slugs = new Set<string>();
  for (const entry of await readdir(root, { withFileTypes: true })) {
    if (
      !entry.isDirectory() ||
      entry.name.startsWith(".") ||
      ["archive", "node_modules", "dist"].includes(entry.name)
    )
      continue;
    const source = join(root, entry.name);
    if (!(await exists(join(source, "slop.ts"), true))) continue;
    const slug = projectSlug(source);
    slugs.add(slug);
    templates.push({ slug, source, bundled: selected.includes(slug) });
  }
  for (const slug of selected)
    if (!slugs.has(slug)) throw new Error(`Unknown bundled template: ${slug}`);
  return templates.sort((a, b) => a.slug.localeCompare(b.slug));
}

export type TemplateInventory = { templates: { slug: string; bundled: boolean }[] };
export function templateInventory(templates: TemplateSource[]): TemplateInventory {
  return { templates: templates.map(({ slug, bundled }) => ({ slug, bundled })) };
}

/** Refuse stale builds when selection or discovered sources changed. */
export async function builtTemplates(): Promise<TemplateInventory> {
  const expected = templateInventory(await discoverTemplates());
  const actual = JSON.parse(
    await readFile(join(repository, "generated/templates/inventory.json"), "utf8"),
  );
  if (JSON.stringify(actual) !== JSON.stringify(expected))
    throw new Error("Template inventory is stale. Run bun run build:templates.");
  return expected;
}
