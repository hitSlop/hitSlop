// The native tools relocated into an app bundle, run with a bare environment: every request
// through the bundled engine, which passes exports to the helper beside it, and no fallback
// to this checkout's page shell or to Bun.
import { afterAll, beforeAll, expect, test } from "bun:test";
import { copyFile, cp, mkdir, mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { prepareNativeFixtures } from "../../scripts/lib/native-fixtures";
import { assertExport, createDocument, engineReply, engineRequest, type Placement } from "../../scripts/lib/native";
import { useTestRegistry } from "../../scripts/lib/artifacts";
useTestRegistry();

let folder: string, document: string, helpers: string;
let placement: Placement;
type Task = { $id: string; text: string; done: boolean; archived: boolean };

beforeAll(async () => {
  await prepareNativeFixtures();
  folder = await mkdtemp(join(tmpdir(), "hitslop-relocated-helper-"));
  helpers = join(folder, "hitSlop.app/Contents/Helpers");
  await mkdir(helpers, { recursive: true });
  const build = resolve("apps/apple/Packages/HitSlopApple/.build/debug");
  for (const name of ["hitslop-native", "HitSlopApple_HitSlopDocument.bundle"])
    await cp(join(build, name), join(helpers, name), { recursive: true });
  const engine = join(folder, "slop-engine");
  await cp(await (await import("../../packages/hitslop/src/cli/engine")).findEngine(), engine);
  placement = {
    engine,
    cwd: folder,
    env: { HITSLOP_NATIVE_CLI: join(helpers, "hitslop-native"), HOME: process.env.HOME, TMPDIR: process.env.TMPDIR, PATH: "/usr/bin:/bin", HITSLOP_TEST_REGISTRY: process.env.HITSLOP_TEST_REGISTRY },
  };
  document = join(folder, "List.slop");
  await createDocument(resolve("generated/native-fixtures/quick-checklist.slop"), document, placement);
}, 120_000);
afterAll(() => rm(folder, { recursive: true, force: true }));

/** A request the relocated tools must refuse, saying `why`. */
async function refused(body: Parameters<typeof engineReply>[0], why: string) {
  const reply = await engineReply(body, placement);
  expect(reply.ok).toBe(false);
  expect(`${!reply.ok && reply.reason} ${!reply.ok && reply.error}`).toContain(why);
}
const value = async () =>
  (await engineRequest({ method: "get", documentPath: document }, placement)).state.value as { title: string; tasks: Task[] };
async function batch(path: string, ops: import("../../packages/hitslop/src/schema/core").Batch["intents"], refusal?: string) {
  const body = { method: "batch" as const, documentPath: path, batch: { intents: ops } };
  if (refusal) await refused(body, refusal);
  else await engineRequest(body, placement);
}

test("a bundled master is never edited", async () => {
  const master = join(folder, "hitSlop.app/Contents/Resources/StarterTemplates/Checklist.slop");
  await mkdir(join(master, ".."), { recursive: true });
  await copyFile(resolve("generated/native-fixtures/quick-checklist.slop"), master);
  const bytes = await readFile(master);
  await batch(master, [{ type: "set", path: ["title"], value: "Must refuse" }], "is_template");
  expect((await readFile(master)).equals(bytes)).toBe(true);
});

test("closed edits and imports go through the relocated engine, keeping row IDs", async () => {
  await batch(document, [{ type: "set", path: ["title"], value: "Relocated native edit" }]);
  expect((await value()).title).toBe("Relocated native edit");
  // Import replaces data with a file's value as its differences, keeping row IDs.
  const state = await value();
  const tasks = [{ text: "Imported", done: false, archived: false }, ...state.tasks.map((task) => ({ ...task, done: true }))];
  await batch(document, [{ type: "replace", path: [], value: { ...state, tasks } }]);
  const imported = await value();
  expect(imported.tasks.slice(1).map((task) => task.$id)).toEqual(state.tasks.map((task) => task.$id));
  expect(imported.tasks.every((task) => task.done || task.text === "Imported")).toBe(true);
  await batch(document, [{ type: "replace", path: ["title"], value: "Imported title" }]);
  expect((await value()).title).toBe("Imported title");
  await batch(document, [{ type: "replace", path: [], value: { title: 5 } }], "type_mismatch");
  expect((await value()).title).toBe("Imported title");
});

test("the relocated helper exports PNG and PDF", async () => {
  for (const format of ["png", "pdf"] as const) {
    const output = join(folder, "export." + format);
    await engineRequest({ method: "export", documentPath: document, format, output }, placement);
    await assertExport(output, format);
  }
}, 120_000);

// Last: it breaks the relocated bundle.
test("a broken relocated page shell refuses to render, with no fallback to this checkout's", async () => {
  const bundle = join(helpers, "HitSlopApple_HitSlopDocument.bundle");
  const shell = [join(bundle, "shell/index.js"), join(bundle, "Contents/Resources/shell/index.js")];
  const index = (await Promise.all(shell.map(async (path) => ((await Bun.file(path).exists()) ? path : undefined)))).find(Boolean);
  expect(index).toBeDefined();
  await rm(index!);
  // Closed state edits need no JS resources; rendering must refuse the broken bundle.
  await value();
  await refused({ method: "export", documentPath: document, format: "pdf", output: join(folder, "broken.pdf") }, "Incomplete page shell");
}, 120_000);
