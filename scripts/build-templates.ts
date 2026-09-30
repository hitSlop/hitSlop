import { buildTemplate } from "../packages/cli/src/template";
import { join, resolve, relative } from "node:path";
import { mkdir, writeFile, rm, rename } from "node:fs/promises";
import { repository, discoverTemplates, templateInventory } from "./templates";
import { sharedTemplateInputs, TemplateCache } from "./template-cache";

export async function buildTemplates(
  output = join(repository, "generated/templates"),
  slugs?: string[],
) {
  const renderer = join(repository, "apps/apple/Packages/HitSlopApple/.build/debug/hitslop-native");
  const discovered = await discoverTemplates();
  const templates = slugs
    ? slugs.map((slug) => {
        const template = discovered.find((template) => template.slug === slug);
        if (!template) throw new Error(`Unknown template fixture: ${slug}`);
        return template;
      })
    : discovered;
  const cache = new TemplateCache(
    resolve(process.env.HITSLOP_TEMPLATE_CACHE_DIR ?? join(repository, ".hitslop/template-cache")),
    await sharedTemplateInputs(
      repository,
      discovered.map((template) => relative(repository, template.source)),
    ),
  );
  const started = performance.now();
  let hits = 0;
  const stage = output + ".building-" + crypto.randomUUID();
  await mkdir(stage, { recursive: true });
  try {
    for (const [index, template] of templates.entries()) {
      const start = performance.now();
      const destination = join(stage, template.slug + ".slop");
      const build = () => buildTemplate(template.source, renderer, destination);
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
    await rm(output, { recursive: true, force: true });
    await rename(stage, output);
    if (!slugs) await cache.prune(templates.map((template) => template.slug));
  } finally {
    await rm(stage, { recursive: true, force: true });
  }
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
