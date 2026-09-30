import { buildProject } from "../packages/cli/src/build";
import { join } from "node:path";
import { repository } from "./templates";
import { buildTemplates } from "./build-templates";
import { buildPresentationFixtures } from "./presentation-fixtures";

export const nativeFixtureSlugs = ["quick-checklist"];

/** Native owners use the active trial template and dedicated presentation fixtures. */
export async function prepareNativeFixtures() {
  await buildTemplates(join(repository, "generated/native-fixtures"), nativeFixtureSlugs);
  await buildProject(
    join(repository, "tests/abi/owner-svelte"),
    join(repository, "generated/abi/owner-svelte.slop"),
  );
  return buildPresentationFixtures();
}
