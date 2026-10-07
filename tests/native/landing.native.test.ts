import { expect, test } from "bun:test";
import { mkdir, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { generateTemplates } from "../../apps/landing/scripts/templates";
import { prepareNativeFixtures } from "../../scripts/lib/native-fixtures";
import { execute } from "../../packages/hitslop/src/cli/engine";
import { png } from "../../packages/hitslop/tests/cli/png-fixture";
import { PackageFormat, RuntimeABI } from "../../packages/hitslop/src/schema/constants";

test("landing generation reads current templates and preserves outputs when a later input fails", async () => {
  await prepareNativeFixtures();
  const destination = await mkdtemp(join(tmpdir(), "hitslop-landing-"));
  const source = { slug: "quick-checklist", file: resolve("generated/native-fixtures/quick-checklist.slop") };
  try {
    const before = await readFile(source.file);
    await generateTemplates([source], destination);
    const output = join(destination, "src/data/templates.json");
    const inventory = await readFile(output, "utf8");
    const [template] = JSON.parse(inventory);
    expect(template.slug).toBe(source.slug);
    expect(template.title.length).toBeGreaterThan(0);
    expect(template.width).toBeGreaterThan(0);
    expect(template.colors.background).toMatch(/^#/);
    const image = join(destination, "public/assets/templates/quick-checklist/preview.png");
    const png = await readFile(image);
    expect([...png.subarray(0, 8)]).toEqual([137, 80, 78, 71, 13, 10, 26, 10]);
    const invalid = join(destination, "invalid.slop");
    await writeFile(invalid, "not a document");
    await expect(generateTemplates([source, { slug: "invalid", file: invalid }], destination)).rejects.toThrow();
    expect(await readFile(output, "utf8")).toBe(inventory);
    expect(await readFile(image)).toEqual(png);
    expect(await readFile(source.file)).toEqual(before);
    expect((await readdir(destination)).some(name => name.startsWith(".templates-"))).toBe(false);
  } finally {
    await rm(destination, { recursive: true, force: true });
  }
}, 120_000);

test("a skinned template without artwork keeps its dimensions and fallback tile", async () => {
  const folder = await mkdtemp(join(tmpdir(), "hitslop-landing-skin-"));
  try {
    const stage = join(folder, "stage");
    await mkdir(stage);
    const image = png(320, 240);
    const key = `media/${createHash("sha256").update(image).digest("hex")}.png`;
    await writeFile(join(stage, "skin.png"), image);
    await writeFile(join(stage, "ui.js"), "export default { mount() { return {}; } };");
    const file = join(folder, "skin.slop");
    await execute({ method: "pack", stage, file, app: {
      packageFormat: PackageFormat, runtimeABI: RuntimeABI,
      declaration: {
        metadata: { slug: "skin", title: "Skin", description: "Landing fixture", author: { name: "hitSlop" }, categories: ["utilities"] },
        window: { kind: "skin", width: 320, height: 240, image: "/assets/" + key },
        document: { kind: "object", properties: {} }, initial: {},
        theme: [{ token: "surface", color: "#112233" }], commands: [], views: { export: false, icon: false },
      },
      roles: { ui: "ui.js", skin: key },
      resources: [
        { kind: "app", key: "ui.js", mediaType: "text/javascript", path: "ui.js" },
        { kind: "app", key, mediaType: "image/png", path: "skin.png" },
      ], artwork: {},
    } });
    await generateTemplates([{ slug: "skin", file }], join(folder, "landing"));
    const [template] = JSON.parse(await readFile(join(folder, "landing/src/data/templates.json"), "utf8"));
    expect(template).toMatchObject({ slug: "skin", shape: "rectangle", width: 320, height: 240, colors: { background: "#112233" } });
    expect(template.preview).toBeUndefined();
    expect(template.icon).toBeUndefined();
  } finally {
    await rm(folder, { recursive: true, force: true });
  }
});
