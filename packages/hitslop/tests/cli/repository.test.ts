import { test, expect } from "bun:test";
import { Database } from "bun:sqlite";
import { mkdir, mkdtemp, readFile, rm, symlink, writeFile, readdir } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { discoverTemplates, templateInventory } from "../../../../scripts/templates/discover";
import { embedTemplates } from "../../../../scripts/templates/embed";
import { assertDocs, assertNoGeneratedSource, assertSkill } from "../../../../scripts/hygiene";
import { stageProject } from "../../src/cli/build";
import { writeTemplate } from "./template-fixture";
import { stageEngines } from "../../../../scripts/build/engines";

test("discovery builds an inventory independently of bundled selection and rejects invalid input", async () => {
  const root = await mkdtemp(join(tmpdir(), "hitslop-discovery-"));
  // Discovery never runs author code: a folder with a slop.ts is a project, named by its slug.
  async function source(name: string) {
    await mkdir(join(root, name), { recursive: true });
    await writeFile(join(root, name, "slop.ts"), 'throw new Error("never evaluated");\n');
  }
  try {
    await writeFile(join(root, "bundled.json"), '["alpha"]');
    await source("alpha");
    await source("beta");
    await mkdir(join(root, "archive"));
    await source("archive/retired");
    await mkdir(join(root, "notes"));
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
    await source("Not A Slug");
    expect((await discoverTemplates(root)).some(project => project.source.endsWith("Not A Slug"))).toBe(true);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("embedding replaces selection and never keeps a deselected starter", async () => {
  const root = await mkdtemp(join(tmpdir(), "hitslop-embedding-"));
  const destination = join(root, "app/StarterTemplates");
  try {
    for (const slug of ["alpha", "beta"]) await writeTemplate(join(root, slug + ".slop"), slug);
    const inventory = (selected: string) => ({
      templates: ["alpha", "beta"].map((slug) => ({ slug, bundled: slug === selected })),
    });
    await embedTemplates(root, destination, inventory("alpha"));
    expect(await readdir(destination)).toEqual(["alpha.slop"]);
    await embedTemplates(root, destination, inventory("beta"));
    expect(await readdir(destination)).toEqual(["beta.slop"]);
    // A document, even one a template became, is never embedded as a starter.
    const document = new Database(join(root, "alpha.slop"));
    document.run("INSERT INTO document(id) VALUES(1)");
    document.close();
    await expect(embedTemplates(root, destination, inventory("alpha"))).rejects.toThrow("Not a template");
    expect(await readdir(destination)).toEqual(["beta.slop"]);
  } finally {
    await Bun.spawn(["/bin/chmod", "-R", "u+w", root]).exited;
    await rm(root, { recursive: true, force: true });
  }
});

test("hygiene allows authored JS and rejects broken skill links", async () => {
  expect(() => assertNoGeneratedSource(["packages/hitslop/src/shell/boot.js"])).not.toThrow();
  expect(() => assertNoGeneratedSource(["packages/hitslop/src/sdk/schema.js"])).toThrow();
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
  const versions = { hitslop: "3.0.0" };
  try {
    await mkdir(join(root, "docs/guides"), { recursive: true });
    await writeFile(join(root, "docs/guides/cli.md"), "# CLI\n");
    const page = (body: string) => writeFile(join(root, "docs/page.md"), body);
    await page("[ok](guides/cli.md#top) [site](/docs/x/) [web](https://example.com) `[code](missing.md)`\n\n```\n[fenced](missing.md)\n```\n");
    await assertDocs(["docs/page.md"], root, versions);
    await page("[gone](../plans/old.md)\n");
    await expect(assertDocs(["docs/page.md"], root, versions)).rejects.toThrow("broken link (../plans/old.md)");
    await page("Run `bunx hitslop@1.2.0 init`.\n");
    await expect(assertDocs(["docs/page.md"], root, versions)).rejects.toThrow("pins hitslop@1.2.0, but the tree is at 3.0.0");
    await page("Run `bunx hitslop@3.0.0 init`.\n");
    await assertDocs(["docs/page.md"], root, versions);
    // A Starlight page links with trailing-slash URLs, which are not files.
    await writeFile(join(root, "docs/site.mdx"), "[next](./getting-started/)\n");
    await assertDocs(["docs/site.mdx"], root, versions);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

// Each complete slop.ts example in the public guides builds with its page's schema.ts.
test("complete public slop.ts examples follow the current contract", async () => {
  // Inside the checkout, so the examples resolve hitslop.
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  let count = 0;
  try {
    for (const file of new Bun.Glob("apps/landing/src/content/**/*.mdx").scanSync(".")) {
      const content = await readFile(file, "utf8");
      const blocks = (title: string) =>
        [...content.matchAll(new RegExp(`\`\`\`ts title="${title}"\n([\\s\\S]*?)\n\`\`\``, "g"))].map((match) => match[1]!);
      const [schema] = blocks("schema.ts");
      for (const slop of blocks("slop.ts")) {
        expect(schema, `${file} shows slop.ts without its schema.ts`).toBeDefined();
        const project = join(root, `example-${++count}`);
        await mkdir(project);
        await writeFile(join(project, "schema.ts"), schema!);
        await writeFile(join(project, "slop.ts"), slop);
        await writeFile(join(project, "App.svelte"), "<h1>Guide example</h1>");
        const [commands] = blocks("commands.ts");
        if (commands) await writeFile(join(project, "commands.ts"), commands);
        await writeFile(join(project, "styles.css"), "");
        await stageProject(project, project + ".stage");
      }
    }
  } finally {
    await rm(root, { recursive: true, force: true });
  }
  expect(count).toBeGreaterThan(0);
}, 30000);

// The CLI finds a staged engine before a checkout's own build, so a refused staging must
// leave nothing behind for a later build to pick up.
test("engine staging refuses a prebuilt engine from another build and leaves nothing", async () => {
  const root = await mkdtemp(join(tmpdir(), "hitslop-engines-"));
  try {
    const fresh = join(root, "slop-engine");
    await writeFile(fresh, "#!/bin/sh\necho build-a\n", { mode: 0o755 });
    const prebuilt = join(root, "prebuilt");
    const other = process.platform === "linux" ? "darwin-arm64" : "linux-x64";
    await mkdir(join(prebuilt, other), { recursive: true });
    await writeFile(join(prebuilt, other, "slop-engine"), "binary");
    await writeFile(join(prebuilt, other, "engine.json"), JSON.stringify({ commit: "c1", buildId: "build-b" }));
    const engines = join(root, "engine");
    await expect(stageEngines(engines, prebuilt, fresh, "c1")).rejects.toThrow("was built from c1 (core build-b)");
    expect((await readdir(root)).sort()).toEqual(["prebuilt", "slop-engine"]);
    // A matching engine stages beside this machine's, each with its provenance.
    await writeFile(join(prebuilt, other, "engine.json"), JSON.stringify({ commit: "c1", buildId: "build-a" }));
    await stageEngines(engines, prebuilt, fresh, "c1");
    expect((await readdir(engines)).sort()).toEqual([`${process.platform}-${process.arch}`, other].sort());
    expect(JSON.parse(await readFile(join(engines, other, "engine.json"), "utf8"))).toEqual({ commit: "c1", buildId: "build-a" });
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
