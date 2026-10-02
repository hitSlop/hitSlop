import { test, expect } from "bun:test";
import { mkdir, mkdtemp, readFile, rm, symlink, writeFile, readdir } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { discoverTemplates, templateInventory } from "../../../scripts/templates";
import { embedTemplates } from "../../../scripts/embed-templates";
import { assertDocs, assertNoGeneratedSource, assertSkill } from "../../../scripts/hygiene";
import { parseManifest } from "../../schema/src/manifest";

test("discovery builds an inventory independently of bundled selection and rejects invalid input", async () => {
  const root = await mkdtemp(join(tmpdir(), "hitslop-discovery-"));
  const base = JSON.parse(await readFile("examples/slops/quick-checklist/manifest.json", "utf8"));
  async function source(name: string, slug = name) {
    await mkdir(join(root, name), { recursive: true });
    await writeFile(join(root, name, "manifest.json"), JSON.stringify({ ...base, slug }));
  }
  try {
    await writeFile(join(root, "bundled.json"), '["alpha"]');
    await source("alpha");
    await source("beta");
    await mkdir(join(root, "archive"));
    await source("archive/retired", "retired");
    expect(templateInventory(await discoverTemplates(root)).templates).toEqual([
      { slug: "alpha", bundled: true },
      { slug: "beta", bundled: false },
    ]);
    await writeFile(join(root, "bundled.json"), '["beta"]');
    expect((await discoverTemplates(root)).filter((t) => t.bundled).map((t) => t.slug)).toEqual([
      "beta",
    ]);
    await writeFile(join(root, "bundled.json"), '["missing"]');
    await expect(discoverTemplates(root)).rejects.toThrow("Unknown bundled");
    await writeFile(join(root, "bundled.json"), '["alpha","alpha"]');
    await expect(discoverTemplates(root)).rejects.toThrow("unique");
    await writeFile(join(root, "bundled.json"), "[]");
    await source("duplicate", "alpha");
    await expect(discoverTemplates(root)).rejects.toThrow("Duplicate template");
    await rm(join(root, "duplicate"), { recursive: true });
    await writeFile(join(root, "beta/manifest.json"), "{}");
    await expect(discoverTemplates(root)).rejects.toThrow("Invalid manifest");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("embedding replaces selection and never keeps a deselected starter", async () => {
  const root = await mkdtemp(join(tmpdir(), "hitslop-embedding-"));
  const destination = join(root, "app/StarterTemplates");
  try {
    for (const slug of ["alpha", "beta"]) {
      await mkdir(join(root, slug + ".slop"));
      await writeFile(
        join(root, slug + ".slop/manifest.json"),
        JSON.stringify({ slug }),
      );
    }
    const inventory = (selected: string) => ({
      templates: ["alpha", "beta"].map((slug) => ({ slug, bundled: slug === selected })),
    });
    await embedTemplates(root, destination, inventory("alpha"));
    expect(await readdir(destination)).toEqual(["alpha.slop"]);
    await embedTemplates(root, destination, inventory("beta"));
    expect(await readdir(destination)).toEqual(["beta.slop"]);
    await mkdir(join(root, "alpha.slop/state"));
    await expect(embedTemplates(root, destination, inventory("alpha"))).rejects.toThrow(
      "Mutable template",
    );
    expect(await readdir(destination)).toEqual(["beta.slop"]);
  } finally {
    await Bun.spawn(["/bin/chmod", "-R", "u+w", root]).exited;
    await rm(root, { recursive: true, force: true });
  }
});

test("hygiene allows authored JS and rejects broken skill links", async () => {
  expect(() => assertNoGeneratedSource(["packages/shell/src/boot.js"])).not.toThrow();
  expect(() => assertNoGeneratedSource(["packages/document/src/schema.js"])).toThrow();
  const root = await mkdtemp(join(tmpdir(), "hitslop-hygiene-"));
  try {
    await symlink("missing", join(root, "skill"));
    await expect(assertSkill("skill/SKILL.md", root)).rejects.toThrow();
    await mkdir(join(root, "missing"));
    await writeFile(
      join(root, "missing/SKILL.md"),
      "---\nname: test\ndescription: Test guide.\n---\n",
    );
    await assertSkill("skill/SKILL.md", root);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("docs must link to files that exist and pin the versions the tree is at", async () => {
  const root = await mkdtemp(join(tmpdir(), "hitslop-docs-"));
  const versions = { cli: "4.0.0", document: "4.0.0", schema: "4.0.0" };
  try {
    await mkdir(join(root, "docs/guides"), { recursive: true });
    await writeFile(join(root, "docs/guides/cli.md"), "# CLI\n");
    const page = (body: string) => writeFile(join(root, "docs/page.md"), body);
    await page("[ok](guides/cli.md#top) [site](/docs/x/) [web](https://example.com) `[code](missing.md)`\n\n```\n[fenced](missing.md)\n```\n");
    await assertDocs(["docs/page.md"], root, versions);
    await page("[gone](../plans/old.md)\n");
    await expect(assertDocs(["docs/page.md"], root, versions)).rejects.toThrow("broken link (../plans/old.md)");
    await page("Run `bunx @hitslop/cli@1.2.0 init`.\n");
    await expect(assertDocs(["docs/page.md"], root, versions)).rejects.toThrow("pins @hitslop/cli@1.2.0, but the tree is at 4.0.0");
    await page("Run `bunx @hitslop/cli@4.0.0 init`.\n");
    await assertDocs(["docs/page.md"], root, versions);
    // A Starlight page links with trailing-slash URLs, which are not files.
    await writeFile(join(root, "docs/site.mdx"), "[next](./getting-started/)\n");
    await assertDocs(["docs/site.mdx"], root, versions);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("complete public manifest examples follow the current contract", async () => {
  let count = 0;
  for (const file of new Bun.Glob("apps/landing/src/content/**/*.mdx").scanSync(".")) {
    const content = await readFile(file, "utf8");
    for (const match of content.matchAll(/```json title="manifest.json"\n([\s\S]*?)\n```/g)) {
      parseManifest(JSON.parse(match[1]!));
      count++;
    }
  }
  expect(count).toBeGreaterThan(0);
});
