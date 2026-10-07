import { exec } from "../../../../scripts/lib/test-process";
// slop.ts declares the app's row: evaluated by the build and by `slop check`, never shipped.
import { test, expect } from "bun:test";
import { mkdtemp, cp, readFile, writeFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { stageProject } from "../../src/cli/build";
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
    const output = join(root, "built");
    const app = await stageProject(source, output);
    expect(app.declaration.metadata.slug).toBe("quick-checklist");
    expect(app.declaration.initial.title).toBe(sentinel);
    expect(await readFile(join(output, "resources/ui.js"), "utf8")).not.toContain(sentinel);
    for (const [name, fields, statements, error] of [
      ["unknown", { lineage: '"future"' }, "", "unknown field"],
      ["field", { title: '""' }, "", "1–80 characters"],
      ["no-initial", { initial: "undefined" }, "", "initial"],
      ["no-document", { document: "undefined" }, "", "defineDocument"],
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
    await stage(misnamed, join(root, "misnamed-stage"));
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
    await expect(stage(source, join(root, "stage"))).rejects.toThrow("Only the generated entry");
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
      const {stderr, code} = await exec([process.execPath, "packages/hitslop/src/cli/cli.ts", "check", source], {timeout: 60_000});
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
