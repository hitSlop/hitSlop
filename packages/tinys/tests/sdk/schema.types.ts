// Compile-time checks of the async author API (typechecked by `bun run check`). Each
// check takes the live document `getDocument(schema)` gives for its definition.
import { defineDocument, s, type Input, type LiveDocument, type Node } from "../../src/sdk/schema";
import type { Node as WireNode } from "../../src/wire/descriptor.generated";

// The SDK may narrow Rust's accepted descriptors, but must never invent a wire shape.
// Its immutable enum tuples are intentionally readonly unlike generated wire arrays.
type DeepReadonly<T> = T extends object ? { readonly [K in keyof T]: DeepReadonly<T[K]> } : T;
type Assert<T extends true> = T;
type DescriptorsStayAligned = Assert<Node extends DeepReadonly<WireNode> ? true : false>;

const counters = defineDocument({ count: s.counter() });
export function counterTypes(doc: LiveDocument<typeof counters.descriptor>) {
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
export async function asyncHandleTypes(doc: LiveDocument<typeof checklist.descriptor>) {
  const { id } = await doc.fields.tasks.insert({ text: "x", done: false });
  await doc.fields.tasks.item(id).done.set(true);
  await doc.fields.title.set("Packing");
  const row = doc.current.tasks[0]!;
  await doc.at(row).done.set(false);
  await doc.fields.tasks.item(row).done.set(true);
  await doc.fields.tasks.move(row, { before: id });
  await doc.fields.tasks.move(id, { after: row });
  await doc.fields.tasks.insert({ text: "next", done: false }, { before: row });
  await doc.fields.tasks.remove(row);
  // @ts-expect-error Row references require a string identity.
  void doc.fields.tasks.item({ $id: 3 });
  // @ts-expect-error Destinations require a row or its ID.
  void doc.fields.tasks.move(row, { before: { text: "no identity" } });
  // Collectors stay synchronous: insert returns its id immediately.
  const minted: string = await doc.change((tx) => {
    const { id } = tx.fields.tasks.insert({ text: "y", done: false });
    tx.fields.tasks.item(id).done.set(true);
    tx.fields.tasks.move(row, { after: id });
    tx.fields.tasks.move(id, { before: row });
    tx.fields.tasks.item(row).text.set("updated");
    tx.fields.tasks.remove(row);
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

// Fields with a default may be left out of inserts and initial values: text, lists,
// records and counters start empty, scalars take their declared default.
const defaults = defineDocument({
  title: s.text(),
  tasks: s.list(s.object({
    text: s.text(),
    done: s.boolean({ default: false }),
    size: s.enum(["s", "l"], { default: "s" }),
    owner: s.string(),
  })),
});
export async function defaultTypes(doc: LiveDocument<typeof defaults.descriptor>) {
  const initial: Input<typeof defaults.descriptor> = { tasks: [{ owner: "Ana" }] };
  await doc.fields.tasks.insert({ owner: "Ana" });
  // @ts-expect-error A field without a default is still required.
  await doc.fields.tasks.insert({ text: "x" });
  // @ts-expect-error A default must be one of the field's values.
  s.enum(["s", "l"], { default: "m" });
  // Reads always hold the field.
  const done: boolean = doc.current.tasks[0]!.done;
  // Text handles read the shown text; typing goes through a binding.
  const title: string = doc.fields.title.value;
  // @ts-expect-error A text handle's value is read-only.
  doc.fields.title.value = "Typed";
  return { initial, done, title };
}

export function undefinedDefaultTypes(possible: boolean | undefined) {
  const required = defineDocument({
    absent: s.boolean({ default: undefined }),
    uncertain: s.boolean({ default: possible }),
  }, { initial: { absent: false, uncertain: true } });
  const complete: Input<typeof required.descriptor> = { absent: false, uncertain: true };
  // @ts-expect-error An undefined default is absent from the emitted descriptor.
  const absent: Input<typeof required.descriptor> = { uncertain: true };
  // @ts-expect-error A possibly-undefined default cannot guarantee omission is valid.
  const uncertain: Input<typeof required.descriptor> = { absent: false };
  return { complete, absent, uncertain };
}

const scalars = defineDocument({
  currency: s.enum(["CAD", "USD"]),
  amount: s.number({ min: 0 }),
  note: s.optional(s.string()),
  photo: s.optional(s.object({ id: s.string() })),
  rows: s.list(s.object({ text: s.text(), limit: s.optional(s.integer()) })),
}, { initial: { currency: "CAD", amount: 0 } });
export async function scalarTypes(doc: LiveDocument<typeof scalars.descriptor>) {
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
  await doc.fields.photo.$set({ id: "a" });
  await doc.fields.photo.id.set("b");
  await doc.fields.photo.$clear();
  // @ts-expect-error An optional object's members start with $, so fields keep every plain name.
  await doc.fields.photo.set({ id: "a" });
  // Optional keys may be omitted from inserts, and so may text, which starts empty.
  await doc.fields.rows.insert({ text: "row" });
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
export async function collectionTypes(doc: LiveDocument<typeof collections.descriptor>) {
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

export function transactionCapabilities(
  doc: LiveDocument<typeof scalars.descriptor>,
  lists: LiveDocument<typeof collections.descriptor>,
) {
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
import * as sdk from "../../src/sdk/schema";
function documentErrors(error: unknown) {
  if (sdk.isDocumentError(error)) { const typed: sdk.DocumentError = error; void typed.code; }
  if (sdk.isRejected(error)) { const code: "rejected" = error.code; void code; }
  // @ts-expect-error The public DocumentError is a type, not a constructor.
  new sdk.DocumentError("rejected", "bad");
}

// One shared write serves `doc.change` in the page and `run({ tx })` in a command.
import type { Tx } from "../../src/sdk/schema";
const wins = defineDocument({ wins: s.counter(), title: s.text() });
const countOne = (tx: Tx<typeof wins>) => tx.fields.wins.increment();
export function sharedWrites(doc: LiveDocument<typeof wins.descriptor>) {
  void doc.change(countOne);
  void doc.change(tx => {
    // @ts-expect-error Tx keeps the document's field types.
    tx.fields.title.set(7);
  });
}

// A definition is data: components acquire the document, captures receive a snapshot.
export function definitionData() {
  const descriptor: { kind: "object" } = wins.descriptor;
  // @ts-expect-error The starting value is build input, not page API.
  void wins.initial;
  // @ts-expect-error A definition has no snapshot; read getDocument(schema).current.
  void wins.current;
  // @ts-expect-error A definition has no handles.
  void wins.fields;
  // @ts-expect-error A definition cannot resolve snapshot objects.
  void wins.at;
  // @ts-expect-error A definition cannot write.
  void wins.change(countOne);
  return descriptor;
}
