import { expect, test } from "bun:test";
import { cp, mkdtemp, readFile, rm, writeFile, symlink } from "node:fs/promises";
import { join } from "node:path";
import { buildTemplate } from "../../src/cli/template";
import { execute, request } from "../../src/cli/engine";
import { HelperProtocol } from "../../src/schema/constants";
import { exec } from "../../src/cli/process";
import { findEngine } from "../../src/cli/engine";
import { evaluate } from "../../src/shell/commands";
import { commandInfo } from "../../src/sdk/commands";
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
export const refuse = doc.command({ description: "Refuse after collecting", args: {}, run({tx}) { tx.fields.title.set("Must roll back"); throw new Error("Deliberate refusal"); } });
export const ambient = doc.command({ description: "No host APIs", args: {}, run() { return [typeof process, typeof fetch, typeof Bun]; } });
`);
    const template = await buildTemplate(source, undefined, join(root, "master.slop"));
    const document = join(root, "Working.slop");
    await execute({ method: "create", from: template, output: document });
    const run = (command: string, args: unknown) => request({ method: "call", documentPath: document, command, args });
    const describe = async () => (await execute({ method: "describe", documentPath: document })).state;
    const first = await describe();
    expect(first.commands.addTask.args.properties.text.type).toBe("string");
    expect(first.fields.find((field: any) => field.path[0] === "title").description).toBeTruthy();
    const added = await run("addTask", { text: "From command" });
    expect(added.ok, JSON.stringify(added)).toBe(true);
    expect(added.ids).toEqual([added.result.id]);
    const before = await describe();
    expect(before.value.tasks.at(-1).text).toBe("From command");
    for (const [name, args, error] of [
      ["addTask", { text: 7 }, "Invalid arguments"],
      ["addTask", { text: "x", unexpected: true }, "Invalid arguments"],
      ["refuse", {}, "Deliberate refusal"],
      ["missing", {}, "No command named"],
    ] as const) {
      const reply = await run(name, args);
      expect(reply.ok).toBe(false);
      expect(reply.error).toContain(error);
      expect((await describe()).value).toEqual(before.value);
    }
    expect((await run("ambient", {})).result).toEqual(["undefined", "undefined", "undefined"]);
    // Direct SDK collection and the restricted child use the same collecting handles.
    const commands = await import(file);
    const spec = commandInfo(commands.archiveFinished)!.spec;
    const local = evaluate({ descriptor: before.schema, value: before.value, args: {}, now: 1, seed: [1,2,3,4] }, spec.run);
    const remote = await run("archiveFinished", {});
    expect(remote.result).toEqual(local.result);
    expect(remote.ok).toBe(true);
    // Unsupported authoring shapes are rejected before replacing the previous build.
    const old = await readFile(template);
    await writeFile(file, (await readFile(file, "utf8")) + "\nexport const notACommand = 7;\n");
    await expect(buildTemplate(source, undefined, template)).rejects.toThrow("must be a command");
    expect(await readFile(template)).toEqual(old);
  } finally { await rm(root, { recursive: true, force: true }); }
}, 120_000);
