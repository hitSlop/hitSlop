// Compile-time checks of the async author API (typechecked by `bun run check`).
import { defineDocument, s, type Input } from "../src/schema";
import type { SlopDocument } from "../src/app/store.svelte";

const counters = defineDocument({ count: s.counter(), optional: s.optional(s.counter()) });
export function counterTypes(doc: SlopDocument<typeof counters.fields.node>) {
  const count: number | null = doc.current.count;
  const optional: number | null | undefined = doc.current.optional;
  // @ts-expect-error Overflow is observable and callers must handle null.
  const unchecked: number = doc.current.count;
  const valid: Input<typeof counters.fields.node> = { count: 1, optional: 2 };
  // @ts-expect-error null is a read fallback, never a valid counter input.
  const invalid: Input<typeof counters.fields.node> = { count: null };
  // @ts-expect-error Counter edits require numbers.
  void doc.fields.count.increment(null);
  return { count, optional, valid, unchecked, invalid };
}

const checklist = defineDocument({
  title: s.text(),
  tasks: s.list(s.object({ text: s.text(), done: s.boolean() })),
});
export async function asyncHandleTypes(doc: SlopDocument<typeof checklist.fields.node>) {
  const { id } = await doc.fields.tasks.insert({ text: "x", done: false });
  await doc.fields.tasks.item(id).done.set(true);
  await doc.fields.title.replace("Packing");
  const row = doc.current.tasks[0]!;
  await doc.at(row).done.set(false);
  // Collectors stay synchronous: insert returns its id immediately.
  const minted: string = await doc.change((tx) => {
    const { id } = tx.fields.tasks.insert({ text: "y", done: false });
    tx.fields.tasks.item(id).done.set(true);
    return id;
  });
  await doc.fields.tasks.remove(minted);
  // @ts-expect-error Writes are asynchronous; the id arrives after acceptance.
  const sync: string = doc.fields.tasks.insert({ text: "z", done: false }).id;
  // @ts-expect-error Text is not a scalar register.
  void doc.at(row).text.set("wrong API");
  // @ts-expect-error Boolean fields require booleans.
  void doc.at(row).done.set("true");
  // @ts-expect-error Required row values cannot be omitted.
  void doc.fields.tasks.insert({ text: "missing done" });
  // @ts-expect-error Plain objects have no snapshot provenance.
  doc.at({ text: "new", done: false });
  return sync;
}
