import { copyFile, cp, mkdir, mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { prepareNativeFixtures } from "./native-fixtures";
import { createDocument, nativeRequest } from "./fixture-documents";
import { strict as assert } from "node:assert";
import { useTestRegistry } from "./runtime-artifacts";
useTestRegistry();

await prepareNativeFixtures();
const folder = await mkdtemp(join(tmpdir(), "hitslop-relocated-helper-"));
try {
  const helpers = join(folder, "hitSlop.app/Contents/Helpers");
  await mkdir(helpers, { recursive: true });
  const build = resolve("apps/apple/Packages/HitSlopApple/.build/debug");
  for (const name of ["hitslop-native", "HitSlopApple_HitSlopDocument.bundle"])
    await cp(join(build, name), join(helpers, name), { recursive: true });
  const template = resolve("generated/native-fixtures/quick-checklist.slop");
  const document = join(folder, "List.slop");
  const placement = {
    cwd: folder,
    env: { HOME: process.env.HOME, TMPDIR: process.env.TMPDIR, PATH: "/usr/bin:/bin", HITSLOP_TEST_REGISTRY: process.env.HITSLOP_TEST_REGISTRY },
  };
  const relocated = join(helpers, "hitslop-native");
  await createDocument(relocated, template, document, placement);
  /** One request, as the CLI sends it; a refusal must mention `refusal`. */
  const request = async (body: Record<string, unknown>, refusal?: string): Promise<any> => {
    const reply = await nativeRequest(relocated, body, placement);
    if (!refusal) assert.ok(reply.ok, reply.error);
    else {
      assert.equal(reply.ok, false, "Expected refusal, but the helper succeeded");
      assert.ok(`${reply.reason} ${reply.error}`.includes(refusal), reply.error);
    }
    return reply;
  };
  const value = async () => (await request({ method: "get", documentPath: document })).state.state.value;
  const batch = (path: string, ops: unknown[], refusal?: string) =>
    request({ method: "batch", documentPath: path, ops: JSON.stringify(ops) }, refusal);
  // A template is never edited: the enclosing app's bundled masters stay as built.
  const master = join(folder, "hitSlop.app/Contents/Resources/StarterTemplates/Checklist.slop");
  await mkdir(join(master, ".."), { recursive: true });
  await copyFile(template, master);
  const masterBytes = await readFile(master);
  await batch(master, [{ type: "set", path: ["title"], value: "Must refuse" }], "create a document from it");
  assert.ok((await readFile(master)).equals(masterBytes), "Bundled master bytes changed");
  await batch(document, [{ type: "set", path: ["title"], value: "Relocated native edit" }]);
  assert.equal((await value()).title, "Relocated native edit");
  // Import replaces data with a file's value as its differences, keeping row IDs.
  const state = await value();
  const tasks = [{ text: "Imported", done: false, archived: false }, ...state.tasks.map((task: any) => ({ ...task, done: true }))];
  await batch(document, [{ type: "replace", path: [], value: { ...state, tasks } }]);
  const imported = await value();
  assert.deepEqual(imported.tasks.slice(1).map((task: any) => task.$id), state.tasks.map((task: any) => task.$id));
  assert.ok(imported.tasks.every((task: any) => task.done || task.text === "Imported"));
  await batch(document, [{ type: "replace", path: ["title"], value: "Imported title" }]);
  assert.equal((await value()).title, "Imported title");
  await batch(document, [{ type: "replace", path: [], value: { title: 5 } }], "type_mismatch");
  assert.equal((await value()).title, "Imported title", "a refused import changes nothing");
  for (const format of ["png", "pdf"]) {
    const output = join(folder, "export." + format);
    await request({ method: "export", documentPath: document, format, output });
    const bytes = await readFile(output);
    assert.ok(bytes.length > 100);
    assert.equal(
      bytes.subarray(0, format === "png" ? 8 : 4).toString("hex"),
      format === "png" ? "89504e470d0a1a0a" : "25504446",
    );
  }
  // A broken relocated resource must fail even while valid checkout resources exist.
  const bundle = join(helpers, "HitSlopApple_HitSlopDocument.bundle");
  const shell = [join(bundle, "shell/index.js"), join(bundle, "Contents/Resources/shell/index.js")];
  const index = (
    await Promise.all(shell.map(async (path) => ((await Bun.file(path).exists()) ? path : undefined)))
  ).find(Boolean);
  assert.ok(index, "Missing embedded page shell");
  await rm(index);
  // Closed state edits need no JS resources. Rendering must refuse the broken bundle.
  await value();
  await request({ method: "export", documentPath: document, format: "pdf", output: join(folder, "broken.pdf") }, "Incomplete page shell");
  console.log(
    "PASS relocated helper: closed editing, PNG/PDF export, no Bun, no checkout page-shell fallback",
  );
} finally {
  await rm(folder, { recursive: true, force: true });
}
