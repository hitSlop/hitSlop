import { expect, test } from "bun:test";
import { cp, mkdtemp, readFile, rm, writeFile, symlink } from "node:fs/promises";
import { join } from "node:path";
import { buildTemplate } from "../../src/cli/template";
import { execute, request } from "../../src/cli/engine";
import { HelperProtocol } from "../../src/schema/constants";
import { exec } from "../../../../scripts/lib/test-process";
import { findEngine } from "../../src/cli/engine";
import { stageProject } from "../../src/cli/build";

test("command imports stay inside the real project when it is reached through a symlink", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-command-link-"));
  try {
    const source = join(root, "source"), alias = join(root, "alias"), stage = join(root, "stage");
    await cp("examples/slops/quick-checklist", source, { recursive: true });
    await symlink(source, alias);
    const input = await stageProject(alias, stage);
    expect(input.declaration.commands.map(c => c.name)).toContain("addTask");
  } finally { await rm(root, { recursive: true, force: true }); }
}, 30_000);

test("stored commands validate arguments, return typed results and refuse atomically", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-commands-"));
  const named = ["--client-protocol", String(HelperProtocol.version)];
  try {
    const source = join(root, "source");
    await cp("examples/slops/quick-checklist", source, { recursive: true });
    const file = join(source, "commands.ts");
    await writeFile(file, (await readFile(file, "utf8")) + `
export const failing = doc.command({ description: "Fail after collecting", args: {}, run({tx}) { tx.fields.title.set("Must roll back"); throw new Error("Deliberate failure"); } });
export const ambient = doc.command({ description: "No host APIs", args: {}, run() { return [typeof process, typeof fetch, typeof Bun]; } });
`);
    const template = await buildTemplate(source, undefined, join(root, "master.slop"));
    const document = join(root, "Working.slop");
    await execute({ method: "create", from: template, output: document });
    const run = (command: string, args: unknown) => request({ method: "call", documentPath: document, command, args });
    const describe = async () => (await execute({ method: "describe", documentPath: document })).state;
    const first = await describe();
    expect(first.commands.addTask.args).toMatchObject({ properties: { text: { type: "string" } } });
    expect(first.fields.find(field => field.path[0] === "title")?.description).toBeTruthy();
    const added = await run("addTask", { text: "From command" });
    expect(added.ok, JSON.stringify(added)).toBe(true);
    if (!added.ok) throw new Error(added.error);
    expect(added.ids).toHaveLength(1);
    expect(added.result).toMatchObject({ id: added.ids[0] });
    const before = await describe();
    expect(before.value).toHaveProperty("tasks");
    const tasks = (before.value as { tasks: unknown[] }).tasks;
    expect(tasks.at(-1)).toMatchObject({ text: "From command" });
    for (const [name, args, error] of [
      ["addTask", { text: 7 }, "Invalid arguments"],
      ["addTask", { text: "x", unexpected: true }, "Invalid arguments"],
      ["failing", {}, "Deliberate failure"],
      ["removeTask", { task: "not an id" }, "Invalid arguments"],
      ["missing", {}, "No command named"],
    ] as const) {
      const reply = await run(name, args);
      expect(reply.ok).toBe(false);
      if (reply.ok) throw new Error(`Expected ${name} to be refused`);
      expect(reply.error).toContain(error);
      expect((await describe()).value).toEqual(before.value);
    }
    const ambient = await run("ambient", {});
    if (!ambient.ok) throw new Error(ambient.error);
    expect(ambient.result).toEqual(["undefined", "undefined", "undefined"]);
    expect((await run("archiveFinished", {})).ok).toBe(true);
    // refuse() and a missing row are messages for the person, under their own reason.
    for (const [name, args, message] of [
      ["addTask", { text: "   " }, "Enter a task."],
      ["restoreTask", { task: "gone" }, "That task no longer exists."],
    ] as const) {
      const reply = await run(name, args);
      if (reply.ok) throw new Error(`Expected ${name} to be refused`);
      expect([reply.error, reply.reason]).toEqual([message, "refused"]);
    }
    // A row argument is resolved from the document; omitted fields took their defaults.
    const restoreArgs = first.commands.restoreTask!.args as { properties: { task: { description: string } } };
    expect(restoreArgs.properties.task.description).toBe("The $id of a row in tasks");
    const id = added.ids[0]!;
    expect(tasks.at(-1)).toMatchObject({ done: false, archived: false });
    expect((await run("restoreTask", { task: id })).ok).toBe(true);
    expect((await run("removeTask", { task: id })).ok).toBe(true);
    expect(((await describe()).value as { tasks: { $id: string }[] }).tasks.some(task => task.$id === id)).toBe(false);
    // Unsupported authoring shapes are rejected before replacing the previous build.
    const old = await readFile(template);
    await writeFile(file, (await readFile(file, "utf8")) + "\nexport const notACommand = 7;\n");
    await expect(buildTemplate(source, undefined, template)).rejects.toThrow("must be a command");
    expect(await readFile(template)).toEqual(old);
  } finally { await rm(root, { recursive: true, force: true }); }
}, 120_000);
