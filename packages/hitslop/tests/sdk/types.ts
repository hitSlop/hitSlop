// Compile-time checks of the async author API (typechecked by `bun run check`). Each
// definition is also the live document authors import, so the checks use `typeof`.
import { defineDocument, s, type Input } from "../../src/sdk/schema";

const counters = defineDocument({ count: s.counter() });
export function counterTypes(doc: typeof counters) {
  const count: number = doc.current.count;
  const valid: Input<typeof counters.descriptor> = { count: 1 };
  // @ts-expect-error A counter's input is a number.
  const invalid: Input<typeof counters.descriptor> = { count: null };
  // @ts-expect-error Counter edits require numbers.
  void doc.fields.count.increment(null);
  return { count, valid, invalid };
}

const checklist = defineDocument({
  title: s.text(),
  tasks: s.list(s.object({ text: s.text(), done: s.boolean() })),
});
export async function asyncHandleTypes(doc: typeof checklist) {
  const { id } = await doc.fields.tasks.insert({ text: "x", done: false });
  await doc.fields.tasks.item(id).done.set(true);
  await doc.fields.title.set("Packing");
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
  // @ts-expect-error Text fields take whole strings.
  void doc.at(row).text.set(7);
  // @ts-expect-error Boolean fields require booleans.
  void doc.at(row).done.set("true");
  // @ts-expect-error Required row values cannot be omitted.
  void doc.fields.tasks.insert({ text: "missing done" });
  // @ts-expect-error Plain objects have no snapshot provenance.
  doc.at({ text: "new", done: false });
  return sync;
}

const scalars = defineDocument({
  currency: s.enum(["CAD", "USD"]),
  amount: s.number({ min: 0 }),
  note: s.optional(s.string()),
  photo: s.optional(s.object({ id: s.string() })),
  rows: s.list(s.object({ text: s.text(), limit: s.optional(s.integer()) })),
});
export async function scalarTypes(doc: typeof scalars) {
  const currency: "CAD" | "USD" = doc.current.currency;
  const note: string | undefined = doc.current.note;
  // @ts-expect-error An optional value may be absent.
  const required: string = doc.current.note;
  await doc.fields.currency.set("USD");
  // @ts-expect-error Enum writes take declared values only.
  await doc.fields.currency.set("EUR");
  await doc.fields.note.clear();
  // @ts-expect-error Required fields cannot be cleared.
  await doc.fields.amount.clear();
  doc.fields.amount.preview(3);
  await doc.fields.photo.set({ id: "a" });
  await doc.fields.photo.id.set("b");
  // Optional keys may be omitted from inserts; required ones may not.
  await doc.fields.rows.insert({ text: "row" });
  // @ts-expect-error Required row values cannot be omitted.
  await doc.fields.rows.insert({ limit: 3 });
  const valid: Input<typeof scalars.descriptor> = { currency: "CAD", amount: 1, rows: [] };
  return { currency, note, required, valid };
}

const collections = defineDocument({
  checkins: s.record(s.integer({ min: 1 })),
  cells: s.record(s.object({ input: s.string() })),
  pixels: s.list(s.string()),
  notes: s.optional(s.text()),
});
export async function collectionTypes(doc: typeof collections) {
  const count: number | undefined = doc.current.checkins["2026-09-23"];
  const pixel: string | undefined = doc.current.pixels[0];
  const notes: string | undefined = doc.current.notes;
  await doc.fields.checkins.put("2026-09-23", 1);
  await doc.fields.checkins.delete("2026-09-23");
  await doc.fields.cells.entry("A1").input.set("x");
  // @ts-expect-error Record values follow the record's value kind.
  await doc.fields.checkins.put("2026-09-23", "one");
  await doc.fields.pixels.insert("#fff", 0);
  await doc.fields.pixels.set(0, "#000");
  doc.fields.pixels.preview(0, "#111");
  await doc.fields.pixels.remove(0, 1);
  await doc.fields.pixels.replace(["#fff"]);
  // @ts-expect-error Scalar lists have no row ids.
  doc.fields.pixels.item("x");
  await doc.fields.notes.set("text");
  await doc.fields.notes.clear();
  return { count, pixel, notes };
}

export function transactionCapabilities(doc: typeof scalars, lists: typeof collections) {
  void doc.change(tx => {
    // @ts-expect-error Previews are live-only, never part of a collector.
    tx.fields.amount.preview(2);
    // @ts-expect-error Binding assignments are live-only.
    tx.fields.amount.value = 2;
  });
  void lists.change(tx => {
    // @ts-expect-error Scalar-list previews are also live-only.
    tx.fields.pixels.preview(0, "red");
  });
}

// Error narrowing is public; its constructor belongs to the separately bundled shell.
import { DocumentError, isDocumentError, isRejected } from "../../src/sdk/schema";
function documentErrors(error: unknown) {
  if (isDocumentError(error)) { const typed: DocumentError = error; void typed.code; }
  if (isRejected(error)) { const code: "rejected" = error.code; void code; }
  // @ts-expect-error The public DocumentError is a type, not a constructor.
  new DocumentError("rejected", "bad");
}
