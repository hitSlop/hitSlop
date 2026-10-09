// Gap: the Rust runner tests prove isolation, not what `run` receives. Oracle: the
// command program's own evaluation of literal requests.
import { expect, test } from "bun:test";
import { evaluate } from "../../src/shell/commands";
import { isRefused } from "../../src/sdk/errors";
import { defineDocument, s, type CommandContext, type Value } from "hitslop";

const doc = defineDocument({ tasks: s.list(s.object({ text: s.text(), done: s.boolean({ default: false }) })) });
const request = (args: unknown) => ({
  value: { tasks: [{ $id: "a", text: "Walk", done: true }] },
  descriptor: doc.descriptor,
  now: 0,
  seed: [1, 2, 3, 4],
  args,
});
const spec = { task: s.row("tasks"), also: s.optional(s.string()) };
type Context = CommandContext<typeof doc.descriptor>;
type Task = Value<typeof doc.descriptor>["tasks"][number];

test("a row argument reaches run as the row from current, so tx.at addresses it", () => {
  let received: unknown;
  const { intents } = evaluate(request({ task: "a" }), (ctx: Context, args: { task: Task }) => {
    received = args.task;
    expect(args.task).toBe(ctx.current.tasks[0]);
    ctx.tx.at(args.task).done.set(false);
  }, spec);
  expect(received).toMatchObject({ $id: "a", text: "Walk" });
  expect(intents).toEqual([{ type: "set", path: ["tasks", { id: "a" }, "done"], value: false }]);
});

test("a row that is gone refuses the command with a message for the person", () => {
  let thrown: unknown;
  try { evaluate(request({ task: "gone" }), () => { throw new Error("run must not start"); }, spec); }
  catch (error) { thrown = error; }
  expect(isRefused(thrown)).toBe(true);
  expect((thrown as Error).message).toBe("That task no longer exists.");
});
