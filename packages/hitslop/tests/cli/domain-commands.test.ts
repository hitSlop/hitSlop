import { expect, test } from "bun:test";
import { cp, mkdtemp, readFile, rm, writeFile, symlink } from "node:fs/promises";
import { join } from "node:path";
import { buildTemplate } from "../../src/cli/template";
import { execute, request } from "../../src/cli/engine";
import { stageProject } from "../../src/cli/build";

test("command imports stay inside the real project when it is reached through a symlink", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-command-link-"));
  try {
    const source = join(root, "source"), alias = join(root, "alias"), stage = join(root, "stage");
    await cp("tests/apps/document", source, { recursive: true });
    await symlink(source, alias);
    const input = await stageProject(alias, stage);
    expect(input.declaration.commands.map(c => c.name)).toContain("addRow");
  } finally { await rm(root, { recursive: true, force: true }); }
}, 30_000);

test("stored commands validate arguments, return typed results and refuse atomically", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-commands-"));
  try {
    const source = join(root, "source");
    await cp("tests/apps/document", source, { recursive: true });
    const file = join(source, "commands.ts");
    await writeFile(file, (await readFile(file, "utf8")).replace("defineCommands(schema, {", `defineCommands(schema, {
      failing: { description: "Fail after collecting", args: {}, run({tx}) { tx.fields.title.set("Must roll back"); throw new Error("Deliberate failure"); } },
      ambient: { description: "No host APIs", args: {}, run() { return [typeof process, typeof fetch, typeof Bun]; } },
    `));
    const template = await buildTemplate(source, join(root, "master.slop"), { renderArtwork: false });
    const document = join(root, "Working.slop");
    await execute({ method: "create", from: template, output: document });
    const run = (command: string, args: unknown) => request({ method: "call", documentPath: document, command, args });
    const describe = async () => (await execute({ method: "describe", documentPath: document })).state;
    const first = await describe();
    expect(first.commands.addRow?.args).toMatchObject({ properties: { text: { type: "string" } } });
    expect(first.fields.find(field => field.path[0] === "title")?.description).toBeTruthy();
    const added = await run("addRow", { text: "From command" });
    expect(added.ok, JSON.stringify(added)).toBe(true);
    if (!added.ok) throw new Error(added.error);
    expect(added.ids).toHaveLength(1);
    expect(added.result).toMatchObject({ id: added.ids[0] });
    const before = await describe();
    expect(before.value).toHaveProperty("tasks");
    const tasks = (before.value as { tasks: unknown[] }).tasks;
    expect(tasks.at(-1)).toMatchObject({ text: "From command" });
    for (const [name, args, error] of [
      ["addRow", { text: 7 }, "Invalid arguments"],
      ["addRow", { text: "x", unexpected: true }, "Invalid arguments"],
      ["failing", {}, "Deliberate failure"],
      ["removeRow", { task: "not an id" }, "Row"],
      ["resetRow", { task: "gone" }, "Row"],
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
    // Authored refusals retain their reason and leave the document unchanged.
    for (const [name, args, message] of [
      ["addRow", { text: "   " }, "Text required."],
    ] as const) {
      const reply = await run(name, args);
      if (reply.ok) throw new Error(`Expected ${name} to be refused`);
      expect([reply.error, reply.reason]).toEqual([message, "refused"]);
    }
    // Scalar IDs address the stored rows; omitted fields took their defaults.
    const restoreArgs = first.commands.resetRow!.args as { properties: { task: { description: string } } };
    expect(restoreArgs.properties.task.description).toBe("The task's $id.");
    const id = added.ids[0]!;
    expect(tasks.at(-1)).toMatchObject({ done: false, archived: false });
    expect((await run("resetRow", { task: id })).ok).toBe(true);
    expect((await run("removeRow", { task: id })).ok).toBe(true);
    expect(((await describe()).value as { tasks: { $id: string }[] }).tasks.some(task => task.$id === id)).toBe(false);
    // Unsupported authoring shapes are rejected before replacing the previous build.
    const old = await readFile(template);
    const program = await readFile(file, "utf8");
    for (const argument of ["s.text()", "s.counter()", "s.object({})", "s.optional(s.object({}))", "s.list(s.string())", "s.record(s.string())"]) {
      await writeFile(file, program.replace("text: s.string()", `text: ${argument}`));
      await expect(buildTemplate(source, template)).rejects.toThrow("invalid_schema");
      expect(await readFile(template)).toEqual(old);
    }
  } finally { await rm(root, { recursive: true, force: true }); }
}, 120_000);
