import { mkdir, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { defineDocument, s } from "../../src/sdk/schema";
import { execute } from "../../src/cli/engine";
import { PackageFormat, RuntimeABI, type AppRow } from "../../src/schema/index";

/** A minimal template file at `output`, packed by the file engine from a stage with preview
 * artwork: what tests of template handling need, without building an app. Returns `output`. */
export async function writeTemplate(output: string, slug = "quick-checklist") {
  const stage = output + ".stage";
  await mkdir(join(stage, "assets"), { recursive: true });
  await mkdir(join(stage, "artwork"));
  const app: AppRow = {
    packageFormat: PackageFormat,
    runtimeABI: RuntimeABI,
    manifest: {
      slug,
      title: "Cache fixture",
      description: "Cache contract",
      author: { name: "hitSlop" },
      categories: ["utilities"],
      presentation: { width: 320, height: 240 },
    },
    descriptor: defineDocument({ title: s.text() }).descriptor,
    initial: { title: "Cache fixture" },
    theme: { accent: "#335577" },
  };
  await writeFile(join(stage, "app.json"), JSON.stringify(app));
  await writeFile(join(stage, "assets/app.js"), "export default { mount() { return {}; } };");
  const png = Buffer.from(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a1ioAAAAASUVORK5CYII=",
    "base64",
  );
  await writeFile(join(stage, "artwork/preview.png"), png);
  try {
    await execute({ method: "pack", stage, file: output });
  } finally {
    await rm(stage, { recursive: true, force: true });
  }
  return output;
}
