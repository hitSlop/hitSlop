import { repository } from "./runtime-artifacts";
import { mkdir, rm } from "node:fs/promises";
import { dirname, join } from "node:path";
import { stageProject } from "../packages/cli/src/build";
import { buildTemplate } from "../packages/cli/src/template";
import { buildTemplates, templateCache } from "./build-templates";
import { buildPresentationFixtures, type Builder } from "./shape-lab";
import { discoverTemplates } from "./templates";

export const nativeFixtureSlugs = ["quick-checklist"];
const output = join(repository, "generated/native-fixtures");

/** Native tests' templates: the active trial template, the ABI owner app and the
 * presentation fixtures, by name. All go through the template cache, so a run that changed
 * none of their inputs only copies them. */
export async function prepareNativeFixtures() {
  await buildTemplates(output, nativeFixtureSlugs);
  const cache = await templateCache("fixtures");
  // Fixtures render no native artwork.
  const build: Builder = async (source, slug, destination) => {
    await mkdir(dirname(destination), { recursive: true });
    await rm(destination, { force: true });
    await cache.build(source, slug, destination, () => buildTemplate(source, undefined, destination), false);
    return destination;
  };
  await build(join(repository, "tests/abi/owner-svelte"), "owner-svelte", join(repository, "generated/abi/owner-svelte.slop"));
  return buildPresentationFixtures(build);
}

/** Each trial template's build stage beside it (`generated/native-fixtures/<slug>`), which
 * benchmarks change before packing. */
export async function stageNativeFixtures() {
  const templates = await discoverTemplates();
  for (const slug of nativeFixtureSlugs) {
    const stage = join(output, slug);
    await rm(stage, { recursive: true, force: true });
    await stageProject(templates.find((template) => template.slug === slug)!.source, stage);
  }
}
