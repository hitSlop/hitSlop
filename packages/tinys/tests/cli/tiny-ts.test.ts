import { exec } from "../../../../scripts/lib/test-process";
// tiny.ts declares the app's row: evaluated by the build and by `tiny check`, never shipped.
import { test, expect } from "bun:test";
import { mkdtemp, cp, readFile, writeFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { stageWorker } from "../../src/cli/build";
import { overrideTiny, overrideSchema, stage } from "./source-fixture";

// tiny.ts declares the app's row: the build evaluates it, checks it as the app opens files,
// and never ships it in the app.
test("tiny.ts declares the app without shipping in app.js and is checked like a stored app", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    const source = join(root, "sentinel");
    await cp("tests/apps/document", source, { recursive: true });
    const sentinel = `tiny-ts-sentinel-${crypto.randomUUID()}`;
    await overrideTiny(source, { description: JSON.stringify(sentinel), capabilities: '["microphone", "camera", "microphone"]' });
    await overrideSchema(source, `{ ...authored.initial, title: "Initial from schema" }`);
    const output = join(root, "built");
    const built = await stageWorker(source, output, "Authoring build failed").done;
    const app = built.input;
    expect(app.declaration.metadata.slug).toBe("document-fixture");
    // Capabilities travel beside the metadata, never in it; acceptance makes them canonical.
    expect(app.declaration.metadata).not.toHaveProperty("capabilities");
    expect(built.capabilities).toEqual(["camera", "microphone"]);
    expect(app.declaration.initial).toMatchObject({ title: "Initial from schema" });
    expect(await readFile(join(output, "resources/ui.js"), "utf8")).not.toContain(sentinel);
    for (const [name, fields, statements, error] of [
      ["unknown", { lineage: '"future"' }, "", "unknown field"],
      ["field", { title: '""' }, "", "1–80 characters"],
      ["theme", { theme: '{ accent: "#ABCDEF" }' }, "", "Theme color"],
      ["capability", { capabilities: '["location"] as never' }, "", "unknown variant `location`"],
    ] as const) {
      const project = join(root, name);
      await cp("tests/apps/document", project, { recursive: true });
      await overrideTiny(project, fields, statements);
      await expect(stage(project, join(root, name + "-stage"))).rejects.toThrow(error);
    }
    const misnamed = join(root, "Fixture With Spaces");
    await cp("tests/apps/document", misnamed, { recursive: true });
    await stage(misnamed, join(root, "misnamed-stage"));
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 60000);

// tiny.ts is build-only: an app that imports it would ship author metadata and initial
// and run build-time code in the page.
test("an app that imports tiny.ts is refused", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    const source = join(root, "imports-tiny");
    await cp("tests/apps/document", source, { recursive: true });
    const app = await readFile(join(source, "App.svelte"), "utf8");
    await writeFile(
      join(source, "App.svelte"),
      app.replace('<script lang="ts">', '<script lang="ts">\nimport tiny from "./tiny";\nconsole.log(tiny.title);'),
    );
    await expect(stage(source, join(root, "stage"))).rejects.toThrow("tiny.ts is metadata");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 60000);

// `tiny check` evaluates tiny.ts as a build does, so a value only the app's rules refuse
// (here a color the types accept) fails the check.
test("tiny check reports tiny.ts errors a build would refuse", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    const source = join(root, "checked");
    await cp("tests/apps/document", source, { recursive: true });
    const check = async () => {
      const {stderr, code} = await exec([process.execPath, "packages/tinys/src/cli/cli.ts", "check", source], {timeout: 60_000});
      return { stderr, code };
    };
    expect((await check()).code).toBe(0);
    await overrideTiny(source, { theme: '{ ...tiny.theme, accent: "#ABCDEF" }' });
    const failed = await check();
    expect(failed.code).not.toBe(0);
    expect(failed.stderr).toContain("Theme color");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 120000);
