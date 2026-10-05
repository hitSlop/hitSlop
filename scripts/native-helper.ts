import { copyFile, cp, mkdir, mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { prepareNativeFixtures } from "./native-fixtures";
import { assertExport, createDocument, helperReply, helperRequest } from "./helper";
import { strict as assert } from "node:assert";
import { useTestRegistry } from "./runtime-artifacts";
useTestRegistry();

await prepareNativeFixtures();
const folder = await mkdtemp(join(tmpdir(), "hitslop-relocated-helper-"));
try {
  const helpers = join(folder, "hitSlop.app/Contents/Helpers");
  await mkdir(helpers, { recursive: true });
  const build = resolve("apps/apple/Packages/HitSlopApple/.build/debug");
  for (const name of ["hitslop-native", "slop-engine", "HitSlopApple_HitSlopDocument.bundle"])
    await cp(join(build, name), join(helpers, name), { recursive: true });
  const template = resolve("generated/native-fixtures/quick-checklist.slop");
  const document = join(folder, "List.slop");
  const placement = {
    helper: join(helpers, "hitslop-native"),
    cwd: folder,
    env: { HOME: process.env.HOME, TMPDIR: process.env.TMPDIR, PATH: "/usr/bin:/bin", HITSLOP_TEST_REGISTRY: process.env.HITSLOP_TEST_REGISTRY },
  };
  const ownerPlacement = { ...placement, helper: join(helpers, "slop-engine") };
  await createDocument(template, document, ownerPlacement);
  /** A request the helper must refuse, saying `why`. */
  const refused = async (body: Parameters<typeof helperReply>[0], why: string) => {
    const reply = await helperReply(body, body.method === "export" ? placement : ownerPlacement);
    assert.equal(reply.ok, false, "Expected refusal, but the helper succeeded");
    assert.ok(`${reply.reason} ${reply.error}`.includes(why), `Expected a refusal for ${why}, got ${reply.error}`);
  };
  type Task = { $id: string; text: string; done: boolean; archived: boolean };
  const value = async () =>
    (await helperRequest({ method: "get", documentPath: document }, ownerPlacement)).state.state.value as { title: string; tasks: Task[] };
  const batch = async (path: string, ops: unknown[], refusal?: string) => {
    const body = { method: "batch" as const, documentPath: path, ops: JSON.stringify(ops) };
    if (refusal) await refused(body, refusal);
    else await helperRequest(body, ownerPlacement);
  };
  // A template is never edited: the enclosing app's bundled masters stay as built.
  const master = join(folder, "hitSlop.app/Contents/Resources/StarterTemplates/Checklist.slop");
  await mkdir(join(master, ".."), { recursive: true });
  await copyFile(template, master);
  const masterBytes = await readFile(master);
  await batch(master, [{ type: "set", path: ["title"], value: "Must refuse" }], "is_template");
  assert.ok((await readFile(master)).equals(masterBytes), "Bundled master bytes changed");
  await batch(document, [{ type: "set", path: ["title"], value: "Relocated native edit" }]);
  assert.equal((await value()).title, "Relocated native edit");
  // Import replaces data with a file's value as its differences, keeping row IDs.
  const state = await value();
  const tasks = [{ text: "Imported", done: false, archived: false }, ...state.tasks.map((task) => ({ ...task, done: true }))];
  await batch(document, [{ type: "replace", path: [], value: { ...state, tasks } }]);
  const imported = await value();
  assert.deepEqual(imported.tasks.slice(1).map((task) => task.$id), state.tasks.map((task) => task.$id));
  assert.ok(imported.tasks.every((task) => task.done || task.text === "Imported"));
  await batch(document, [{ type: "replace", path: ["title"], value: "Imported title" }]);
  assert.equal((await value()).title, "Imported title");
  await batch(document, [{ type: "replace", path: [], value: { title: 5 } }], "type_mismatch");
  assert.equal((await value()).title, "Imported title", "a refused import changes nothing");
  for (const format of ["png", "pdf"] as const) {
    const output = join(folder, "export." + format);
    await helperRequest({ method: "export", documentPath: document, format, output }, placement);
    await assertExport(output, format);
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
  await refused({ method: "export", documentPath: document, format: "pdf", output: join(folder, "broken.pdf") }, "Incomplete page shell");
  console.log(
    "PASS relocated native tools: Rust closed editing, PNG/PDF export, no Bun, no checkout page-shell fallback",
  );
} finally {
  await rm(folder, { recursive: true, force: true });
}
