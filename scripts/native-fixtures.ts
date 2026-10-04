import { buildTemplate } from "../packages/cli/src/template";
import { join } from "node:path";
import { repository } from "./templates";
import { buildTemplates } from "./build-templates";
import { buildPresentationFixtures } from "./presentation-fixtures";

export const nativeFixtureSlugs = ["quick-checklist"];

/** Native owners use the active trial template and dedicated presentation fixtures. Each
 * template's stage stays beside it, for tests that change an app before packing it. */
export async function prepareNativeFixtures() {
  const output = join(repository, "generated/native-fixtures");
  await buildTemplates(output, nativeFixtureSlugs, true);
  await buildTemplate(
    join(repository, "tests/abi/owner-svelte"),
    undefined,
    join(repository, "generated/abi/owner-svelte.slop"),
  );
  return buildPresentationFixtures();
}
