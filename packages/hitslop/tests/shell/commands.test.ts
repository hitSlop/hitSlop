// The runner receives checked scalar arguments and collects atomic intents.
import { expect, test } from "bun:test";
import { evaluate } from "../../src/shell/commands";
import { defineDocument, s, type CommandContext } from "hitslop";
import { defineCommands } from "../../src/sdk/commands";
const doc = defineDocument({ tasks: s.list(s.object({ text: s.text(), done: s.boolean({ default: false }) })) });
const request = (args: unknown) => ({ value: { tasks: [{ $id: "a", text: "Walk", done: true }] }, descriptor: doc.descriptor, now: 0, seed: [1, 2, 3, 4], args });
test("scalar IDs address rows and arguments and snapshots are immutable", () => {
  const { intents } = evaluate(request({ task: "a" }), (ctx: CommandContext<typeof doc.descriptor>, args) => {
    expect(Object.isFrozen(args)).toBe(true);
    expect(Object.isFrozen(ctx.current.tasks)).toBe(true);
    ctx.tx.fields.tasks.item(args.task).done.set(false);
  });
  expect(intents).toEqual([{ type: "set", path: ["tasks", { id: "a" }, "done"], value: false }]);
});
test("untyped commands reject noncallable, async and generator implementations", () => {
  for (const run of [null, 7, async () => 1, function*() { yield 1; }, async function*() { yield 1; }])
    expect(() => defineCommands(doc, {probe:{description:"Probe",args:{},run:run as any}})).toThrow();
});
test("the runner rejects thenables and non-JSON results after collecting", () => {
  for (const result of [Promise.resolve(1), {then() {}}, 1n, NaN, {value: Symbol()}])
    expect(() => evaluate(request({}), (ctx: CommandContext<typeof doc.descriptor>) => { ctx.tx.fields.tasks.remove("a"); return result; })).toThrow();
});
