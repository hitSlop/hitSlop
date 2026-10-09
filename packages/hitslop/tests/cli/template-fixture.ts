import { png } from "./png-fixture";
import { mkdir, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { defineDocument, s } from "../../src/sdk/schema";
import { execute } from "../../src/cli/engine";
import { PackageFormat, RuntimeABI } from "../../src/schema/constants";

/** A minimal template file at `output`, packed by the file engine from a stage with preview
 * artwork: what tests of template handling need, without building an app. Returns `output`. */
export async function writeTemplate(output: string, slug = "fixture-one") {
  const stage = output + ".stage";
  await mkdir(join(stage, "assets"), { recursive: true });
  await mkdir(join(stage, "artwork"));
  const app = {
    packageFormat: PackageFormat,
    runtimeABI: RuntimeABI,
    declaration: { metadata: {
      slug,
      title: "Cache fixture",
      description: "Cache contract",
      author: { name: "hitSlop" },
      categories: ["utilities"],
    },
    window: { kind: "standard", width: 320, height: 240 },
    document: defineDocument({ title: s.text() }).descriptor,
    initial: { title: "Cache fixture" },
    theme: [{ token: "accent", color: "#335577" }], commands: [], views: { export: false, icon: false } },
    roles: { ui: "ui.js" },
    resources: [{ kind: "app", key: "ui.js", mediaType: "text/javascript", path: "assets/ui.js" }],
    artwork: { preview: "artwork/preview.png" },
  };
  await writeFile(join(stage, "assets/ui.js"), "export default { mount() { return {}; } };");
  await writeFile(join(stage, "artwork/preview.png"), png(1, 1));
  try {
    await execute({ method: "pack", stage, file: output, app });
  } finally {
    await rm(stage, { recursive: true, force: true });
  }
  return output;
}
