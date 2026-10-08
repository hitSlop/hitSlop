// Gap: `draft` is Svelte rune code, compiled into each slop. Oracle: the compiled module
// driven through a command whose acceptance the test controls.
import { afterAll, expect, test } from "bun:test";
import { compileModule } from "svelte/compiler";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";

const folder = await mkdtemp(join(import.meta.dir, ".build-test-draft-"));
afterAll(() => rm(folder, { recursive: true, force: true }));
const source = await Bun.file(new URL("../../src/sdk/app/draft.svelte.ts", import.meta.url)).text();
const javascript = new Bun.Transpiler({ loader: "ts" }).transformSync(source);
await writeFile(join(folder, "draft.svelte.js"), compileModule(javascript, { generate: "client", filename: "draft.svelte.js" }).js.code);
const { draft } = (await import(join(folder, "draft.svelte.js"))) as typeof import("../../src/sdk/app/draft.svelte");

function controlled() {
  const calls: { text: string; settle: (ok: boolean) => void }[] = [];
  const command = (args: { text: string }) =>
    new Promise<string>((resolve, reject) =>
      calls.push({ text: args.text, settle: (ok) => (ok ? resolve(`id:${args.text}`) : reject(new Error("refused"))) }));
  return { calls, task: draft((text) => command({ text })) };
}

test("a draft sends trimmed text once and clears only when accepted and unchanged", async () => {
  const { calls, task } = controlled();
  expect(task.ready).toBe(false);
  task.value = "  Walk  ";
  expect(task.ready).toBe(true);
  const first = task.submit();
  expect(await task.submit()).toBeUndefined();
  expect([calls.length, calls[0]!.text, task.pending, task.ready]).toEqual([1, "Walk", true, false]);
  calls[0]!.settle(true);
  expect(await first).toBe("id:Walk");
  expect([task.value, task.pending]).toEqual(["", false]);
  // Typing while a submission runs keeps the newer text.
  task.value = "Read";
  const second = task.submit();
  task.value = "Read a book";
  calls[1]!.settle(true);
  await second;
  expect(task.value).toBe("Read a book");
});

test("a refused submission keeps the text and rejects to the caller", async () => {
  const { calls, task } = controlled();
  task.value = "Walk";
  const submitted = task.submit();
  calls[0]!.settle(false);
  await expect(submitted).rejects.toThrow("refused");
  expect([task.value, task.pending]).toEqual(["Walk", false]);
  task.value = "   ";
  expect(await task.submit()).toBeUndefined();
  expect(calls).toHaveLength(1);
});

test("a submitter that throws releases pending state and preserves the draft", async () => {
  const task = draft(() => { throw new Error("Could not submit"); });
  task.value = "Keep this";
  await expect(task.submit()).rejects.toThrow("Could not submit");
  expect(task.value).toBe("Keep this");
  expect(task.pending).toBe(false);
  expect(task.ready).toBe(true);
});
