import { defineDocument, s } from "../../src/sdk/schema";
import { defineCommands } from "../../src/sdk/commands";
const doc = defineDocument({ title: s.text(), count: s.counter() });
const commands = defineCommands(doc, {
  rename: {
    description: "Rename", args: { title: s.string(), by: s.number({ default: 1 }), note: s.optional(s.string()), lane: s.enum(["a", "b"]) },
    run({ tx, current }, { title, by, note, lane }) {
      const text: string = title, amount: number = by, optional: string | undefined = note, choice: "a" | "b" = lane;
      tx.fields.title.set(text);
      tx.fields.count.increment(amount);
      // @ts-expect-error Document fields retain their types.
      tx.fields.title.set(7);
      return { title, by, lane: choice, count: current.count, note: optional ?? "" };
    },
  },
});
// @ts-expect-error Commands are not page callables.
commands.rename({ title: "Next" });
// @ts-expect-error Commands cannot be async.
defineCommands(doc, { later: { description: "Later", args: {}, async run() {} } });
// @ts-expect-error Commands cannot return a promise.
defineCommands(doc, { later: { description: "Later", args: {}, run: () => Promise.resolve(1) } });
// @ts-expect-error Commands cannot be generators.
defineCommands(doc, { later: { description: "Later", args: {}, *run() { yield 1; } } });
// @ts-expect-error Commands accept only scalar arguments.
defineCommands(doc, { nested: { description: "Nested", args: { inner: s.object({ title: s.string() }) }, run() {} } });
// @ts-expect-error Text is not a scalar command argument.
defineCommands(doc, { text: { description: "Text", args: { title: s.text() }, run() {} } });
// @ts-expect-error Lists are not command arguments.
defineCommands(doc, { list: { description: "List", args: { list: s.list(s.string()) }, run() {} } });
