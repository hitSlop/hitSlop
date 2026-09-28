import { defineDocument, s, type Input } from "../src/schema";
import type { Document } from "../src/document";
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
  // @ts-expect-error Optional counter inputs still require a number when present.
  const invalidOptional: Input<typeof counters.fields.node> = { count: 0, optional: null };
  // @ts-expect-error Counter edits require numbers.
  doc.fields.count.increment(null);
  return { count, optional, valid };
}
const schema = defineDocument({
  title: s.text(),
  currency: s.enum(["CAD", "USD"]),
  tasks: s.list(
    s.object({
      text: s.text(),
      done: s.boolean(),
      note: s.optional(s.string()),
      amount: s.number(),
    }),
  ),
});
export function authoringTypes(doc: Document<typeof schema.fields.node>) {
  doc.fields.title.replace("Fine");
  doc.fields.currency.set("CAD");
  doc.fields.tasks.insert({ text: "x", done: false, amount: 100 });
  doc.fields.tasks.item("id").note.clear();
  // @ts-expect-error Text is not a scalar register
  doc.fields.title.set("wrong API");
  // @ts-expect-error Unknown enum variant
  doc.fields.currency.set("GBP");
  // @ts-expect-error Only optional scalars can be cleared
  doc.fields.currency.clear();
  // @ts-expect-error Nested boolean requires boolean
  doc.fields.tasks.item("id").done.set("true");
  // @ts-expect-error Required values cannot be omitted
  doc.fields.tasks.insert({ text: "Missing done" });
  s.optional(s.text());
  s.list(s.string());
  // @ts-expect-error Lists hold rows or scalars, not text
  s.list(s.text());
}

export function handleTypes(doc: Document<typeof schema.fields.node>) {
  const { title, currency, tasks } = doc.fields;
  title.replace("Packing");
  currency.set("USD");
  const { id } = tasks.insert({ text: "x", done: false, amount: 100 });
  tasks.item(id).note.set("optional");
  tasks.item(id).note.clear();
  const inserted: string = doc.change((tx) => {
    const { id } = tx.fields.tasks.insert({ text: "x", done: false, amount: 100 });
    tx.fields.tasks.item(id).done.set(true);
    return id;
  });
  tasks.remove(inserted);
  // @ts-expect-error Text handles do not expose scalar set
  title.set("wrong");
  // @ts-expect-error Scalars do not expose replace
  tasks.item(id).done.replace("wrong");
  // @ts-expect-error Clear is only on optional scalar handles
  currency.clear();
  // @ts-expect-error Invalid enum variant
  currency.set("GBP");
  // @ts-expect-error Explicit clear rather than undefined assignment
  tasks.item(id).note.set(undefined);
  // @ts-expect-error Required row values cannot be omitted
  tasks.insert({ text: "missing values" });
  // @ts-expect-error Handle values are not readable snapshots
  tasks.item(id).done.value;
}

export function snapshotHandleTypes(doc: Document<typeof schema.fields.node>, ui: SlopDocument<typeof schema.fields.node>) {
  const row = doc.current.tasks[0]!;
  doc.at(row).text.replace("Emoji 😁");
  doc.at(row).done.set(true);
  doc.at(doc.current).currency.set("CAD");
  ui.at(ui.current.tasks[0]!).done.set(false);
  // @ts-expect-error Text is not a scalar, even though both project to strings.
  doc.at(row).text.set("wrong API");
  // @ts-expect-error Field names remain schema checked.
  doc.at(row).dnoe.set(true);
  // @ts-expect-error Boolean field requires a boolean.
  doc.at(row).done.set("true");
  // @ts-expect-error Enum variants remain checked.
  doc.at(doc.current).currency.set("GBP");
  // @ts-expect-error Plain objects have no snapshot provenance.
  doc.at({ text: "new", done: false });
  // @ts-expect-error The Svelte adapter preserves handle types.
  ui.at(ui.current.tasks[0]!).amount.set("10");
  doc.change(tx => {
    tx.at(row).note.clear();
    // @ts-expect-error Transaction handles are typed too.
    tx.at(row).done.set(1);
  });
  ui.change(tx => {
    // @ts-expect-error Svelte transactions preserve enum variants.
    tx.at(ui.current).currency.set("GBP");
  });
}

const nestedSchema = defineDocument({
  detail: s.optional(s.object({ label: s.text(), code: s.string() })),
  cells: s.record(s.object({ value: s.integer() })),
  outline: s.tree(s.object({ title: s.text() })),
});
export function nestedSnapshotTypes(doc: Document<typeof nestedSchema.fields.node>) {
  if (doc.current.detail) {
    doc.at(doc.current.detail).label.replace("Title");
    doc.at(doc.current.detail).code.set("ID");
    doc.at(doc.current.detail).clear();
    // @ts-expect-error Scalar strings keep scalar handles.
    doc.at(doc.current.detail).code.replace("wrong");
  }
  doc.at(doc.current.cells).put("A1", { value: 1 });
  doc.at(doc.current.cells.A1!).value.set(2);
  doc.at(doc.current.outline[0]!.children[0]!).title.replace("Nested");
  // @ts-expect-error Synthetic tree children are not stored fields.
  doc.at(doc.current.outline[0]!).children;
}
