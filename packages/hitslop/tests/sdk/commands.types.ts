import { defineDocument, s } from "../../src/sdk/schema";
const doc = defineDocument({ title: s.text(), count: s.counter() });
const rename = doc.command({ description: "Rename", args: { title: s.string() }, run({ tx, current }, { title }) { tx.fields.title.set(title); return current.count; } });
function callers() {
  const result: Promise<number> = rename({ title: "Next" });
  // @ts-expect-error Required command arguments cannot be omitted.
  rename();
  // @ts-expect-error Argument inference comes from s.* descriptors.
  rename({ title: 7 });
  // @ts-expect-error Unknown fields are not arguments.
  rename({ title: "Next", other: true });
  return result;
}
// Commands run synchronously in the owner; an awaited write would fall outside the batch.
// @ts-expect-error run cannot be async.
doc.command({ description: "Later", args: {}, async run({ tx }) { tx.fields.count.increment(); } });
// @ts-expect-error run cannot return a promise.
doc.command({ description: "Later", args: {}, run: () => Promise.resolve(1) });

// Row arguments: callers pass a row or its $id; run receives the row from `current`.
const tasks = defineDocument({
  tasks: s.list(s.object({ text: s.text(), done: s.boolean({ default: false }) })),
  tags: s.list(s.string()),
});
type IsNever<T> = [T] extends [never] ? true : false;
const restore = tasks.command({
  description: "Restore",
  args: { task: s.row("tasks"), note: s.string({ default: "" }) },
  run({ tx }, { task, note }) {
    const resolved: IsNever<typeof task> = false;
    const text: string = task.text;
    const filled: string = note;
    tx.at(task).done.set(false);
    return { resolved, text, filled };
  },
});
const missing = tasks.command({
  description: "A row of a list that is not there",
  args: { task: s.row("nothing") },
  run(_, { task }) {
    // The core refuses the declaration at check and build; the type is never.
    const unresolved: IsNever<typeof task> = true;
    return unresolved;
  },
});
async function rowCallers(row: (typeof tasks.current.tasks)[number]) {
  await restore({ task: row });
  await restore({ task: row.$id, note: "back" });
  // @ts-expect-error A row argument takes a row or its $id.
  await restore({ task: 7 });
  // @ts-expect-error Row arguments are required unless optional.
  await restore({});
  return missing;
}
void rowCallers;
// Rows resolve only as required top-level arguments; nesting one is refused in the types
// as at pack.
tasks.command({
  description: "Nested row",
  // @ts-expect-error A row cannot sit inside an object argument.
  args: { pick: s.object({ task: s.row("tasks") }) },
  run() {},
});
tasks.command({
  description: "Optional row",
  // @ts-expect-error A row cannot be optional.
  args: { task: s.optional(s.row("tasks")) },
  run() {},
});
tasks.command({
  description: "Row list",
  // @ts-expect-error A list item cannot be a row.
  args: { tasks: s.list(s.row("tasks")) },
  run() {},
});
