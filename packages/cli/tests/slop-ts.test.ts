// slop.ts declares the app's row: evaluated by the build and by `slop check`, never shipped.
import { test, expect } from "bun:test";
import { mkdtemp, cp, readFile, writeFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { overrideSlop, stage } from "./source-fixture";

// slop.ts declares the app's row: the build evaluates it, checks it as the app opens files,
// and never ships it in the app.
test("slop.ts declares the app without shipping in app.js and is checked like a stored app", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    const source = join(root, "sentinel");
    await cp("examples/slops/quick-checklist", source, { recursive: true });
    const sentinel = `slop-ts-sentinel-${crypto.randomUUID()}`;
    await overrideSlop(source, { initial: `{ ...slop.initial, title: ${JSON.stringify(sentinel)} }` });
    const output = await stage(source, join(root, "built"));
    const app = JSON.parse(await readFile(join(output, "app.json"), "utf8"));
    expect(app.manifest.slug).toBe("sentinel");
    expect(app.initial.title).toBe(sentinel);
    expect(await readFile(join(output, "assets/app.js"), "utf8")).not.toContain(sentinel);
    for (const [name, fields, statements, error] of [
      ["mismatch", { schema: "defineDocument({ title: s.text() })" }, 'import { defineDocument, s } from "@hitslop/document";', "slop.ts: schema must be schema.ts's default export"],
      ["unknown", { lineage: '"future"' }, "", 'Invalid manifest at /'],
      ["field", { title: '""' }, "", "Invalid manifest at /title"],
      ["no-initial", { initial: "undefined" }, "", "slop.ts: initial is required"],
      ["no-theme", { theme: "undefined" }, "", "slop.ts: theme is required"],
      ["css-import", {}, 'import "./styles.css";', "slop.ts: styles.css cannot be imported here"],
      ["outside-import", {}, 'import "../sentinel/schema";', "slop.ts: ../sentinel/schema.ts is outside the project"],
      ["slug", { slug: '"other-slug"' }, "", "slop.ts: remove slug"],
      ["initial", { initial: "{ ...slop.initial, title: 42 }" }, "", "type_mismatch"],
      ["theme", { theme: '{ accent: "#ABCDEF" }' }, "", "Theme color"],
    ] as const) {
      const project = join(root, name);
      await cp("examples/slops/quick-checklist", project, { recursive: true });
      await overrideSlop(project, fields, statements);
      await expect(stage(project, join(root, name + "-stage"))).rejects.toThrow(error);
    }
    const misnamed = join(root, "Quick Checklist");
    await cp("examples/slops/quick-checklist", misnamed, { recursive: true });
    await expect(stage(misnamed, join(root, "misnamed-stage"))).rejects.toThrow("folder's name is its slug");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 60000);

// slop.ts is build-only: an app that imports it would ship author metadata and initial
// values, and run build-time code in the page.
test("an app that imports slop.ts is refused", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    const source = join(root, "imports-slop");
    await cp("examples/slops/quick-checklist", source, { recursive: true });
    const app = await readFile(join(source, "App.svelte"), "utf8");
    await writeFile(
      join(source, "App.svelte"),
      app.replace('<script lang="ts">', '<script lang="ts">\nimport slop from "./slop";\nconsole.log(slop.initial.title);'),
    );
    await expect(stage(source, join(root, "stage"))).rejects.toThrow("slop.ts is build-only");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 60000);

// `slop check` evaluates slop.ts as a build does, so a value only the app's rules refuse
// (here a color the types accept) fails the check.
test("slop check reports slop.ts errors a build would refuse", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    const source = join(root, "checked");
    await cp("examples/slops/quick-checklist", source, { recursive: true });
    const check = async () => {
      const child = Bun.spawn([process.execPath, "packages/cli/src/cli.ts", "check", source], { stdout: "ignore", stderr: "pipe" });
      const [stderr, code] = await Promise.all([new Response(child.stderr).text(), child.exited]);
      return { stderr, code };
    };
    expect((await check()).code).toBe(0);
    await overrideSlop(source, { theme: '{ ...slop.theme, accent: "#ABCDEF" }' });
    const failed = await check();
    expect(failed.code).not.toBe(0);
    expect(failed.stderr).toContain("Theme color");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 120000);
