import { repository } from "../lib/artifacts";
import { buildTemplate } from "../../packages/hitslop/src/cli/template";
import { join, resolve, relative } from "node:path";
import { mkdir, writeFile } from "node:fs/promises";
import { discoverTemplates, templateInventory } from "./discover";
import { sharedTemplateInputs, TemplateCache } from "./cache";
import { debugHelper } from "../lib/native";
import { publishFolder } from "../lib/artifacts";

/** The template cache (`HITSLOP_TEMPLATE_CACHE_DIR`, or `.hitslop/template-cache`), keyed by
 * what every build reads; `folder` keeps a separate set of entries there. */
export async function templateCache(folder = "") {
  const discovered = await discoverTemplates();
  return new TemplateCache(
    join(resolve(process.env.HITSLOP_TEMPLATE_CACHE_DIR ?? join(repository, ".hitslop/template-cache")), folder),
    await sharedTemplateInputs(
      repository,
      discovered.map((template) => relative(repository, template.source)),
    ),
  );
}

/** Builds `slugs` (every discovered template by default) into `output`, through the
 * template cache. */
export async function buildTemplates(output = join(repository, "generated/templates"), slugs?: string[]) {
  const discovered = await discoverTemplates();
  const templates = slugs
    ? slugs.map((slug) => {
        const template = discovered.find((template) => template.slug === slug);
        if (!template) throw new Error(`Unknown template fixture: ${slug}`);
        return template;
      })
    : discovered;
  const cache = await templateCache();
  const started = performance.now();
  let hits = 0;
  await publishFolder(output, async (stage) => {
    for (const [index, template] of templates.entries()) {
      const start = performance.now();
      const destination = join(stage, template.slug + ".slop");
      // Discovery names a repository template by its folder; the app declares its own slug.
      const named = (slug: string) => {
        if (slug !== template.slug) throw new Error(`examples/slops/${template.slug} declares slug "${slug}"; rename the folder or the slug`);
        return destination;
      };
      const build = () => buildTemplate(template.source, { env: { ...process.env, HITSLOP_NATIVE_CLI: debugHelper } }, named);
      console.log(`Preparing template ${index + 1}/${templates.length}: ${template.slug}`);
      const status = await cache.build(template.source, template.slug, destination, build);
      if (status === "hit") hits++;
      const reason = cache.misses.get(template.slug);
      console.log(
        `Template ${index + 1}/${templates.length}: ${template.slug} — ${status}${reason ? ` (${reason.join(", ")})` : ""} (${((performance.now() - start) / 1000).toFixed(1)}s)`,
      );
    }
    if (!slugs)
      await writeFile(
        join(stage, "inventory.json"),
        JSON.stringify(templateInventory(templates), null, 2) + "\n",
      );
  });
  if (!slugs) await cache.prune(templates.map((template) => template.slug));
  const seconds = (performance.now() - started) / 1000;
  console.log(
    `Templates: ${hits} cache hits, ${templates.length - hits} built in ${seconds.toFixed(1)}s`,
  );
  // Cross-run reuse is only proven by recorded hits, so retain the report with CI evidence.
  const evidence = join(repository, ".hitslop/evidence");
  await mkdir(evidence, { recursive: true });
  await writeFile(
    join(evidence, `template-cache-${slugs ? "fixtures" : "templates"}.json`),
    JSON.stringify(
      { hits, built: templates.length - hits, seconds, misses: Object.fromEntries(cache.misses) },
      null,
      2,
    ) + "\n",
  );
}

if (import.meta.main) await buildTemplates();
