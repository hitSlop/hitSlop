// Scalar kinds through the SDK and the real WASM core. Failure: a handle that writes the
// wrong intent, a preview that leaks into history or outlives its write, or a form
// control that sends an invalid value. Oracle: literal snapshots and the core's state.
import { expect, test } from "bun:test";
import { OwnerDocument } from "../src/owner/document";
import { wasmTransport, type OwnerTransport } from "../src/owner/transport";
import { defineDocument, s } from "../src/schema";
const wasm = await import(new URL("../../../generated/v1/core/wasm/hitslop_core_wasm.js", import.meta.url).href);
wasm.initSync({
  module: await Bun.file(new URL("../../../generated/v1/core/wasm/hitslop_core_wasm_bg.wasm", import.meta.url)).bytes(),
});

const definition = defineDocument({
  currency: s.enum(["CAD", "USD", "EUR"]),
  label: s.string({ maxLength: 4 }),
  ratio: s.number({ min: 0, max: 1 }),
  rating: s.integer({ min: 1, max: 5 }),
  memo: s.optional(s.string()),
  photo: s.optional(s.object({ id: s.string(), name: s.string() })),
  rows: s.list(s.object({ text: s.text(), amount: s.number(), note: s.optional(s.string()) })),
});
const initial = { currency: "CAD", label: "ab", ratio: 0.5, rating: 3, rows: [{ $id: "r1", text: "Coffee", amount: 475 }] };

async function open() {
  const core = wasm.WasmDocument.create(JSON.stringify(definition.descriptor), JSON.stringify(initial));
  const errors: unknown[] = [];
  const transport = wasmTransport(core) as OwnerTransport & Record<string, any>;
  const doc = await OwnerDocument.open(definition, transport, (error) => errors.push(error));
  const saved = () => JSON.parse(core.snapshot()).value;
  return { core, doc, errors, saved };
}

test("scalar, optional and optional-object handles write through the core", async () => {
  const { core, doc, saved } = await open();
  try {
    await doc.fields.currency.set("EUR");
    await doc.fields.ratio.set(0.25);
    await doc.fields.rating.set(5);
    await doc.fields.memo.set("hi");
    await doc.fields.photo.set({ id: "a", name: "first" });
    await doc.fields.photo.name.set("renamed");
    const { id } = await doc.fields.rows.insert({ text: "Train", amount: 12.5 });
    await doc.fields.rows.item(id).note.set("work");
    await doc.fields.memo.clear();
    expect(saved()).toEqual({
      currency: "EUR", label: "ab", ratio: 0.25, rating: 5,
      photo: { id: "a", name: "renamed" },
      rows: [{ $id: "r1", text: "Coffee", amount: 475 }, { $id: id, text: "Train", amount: 12.5, note: "work" }],
    });
    expect(JSON.parse(JSON.stringify(doc.current))).toEqual(saved());
    expect("memo" in doc.current).toBe(false);
    await doc.fields.photo.clear();
    expect(doc.current.photo).toBeUndefined();
  } finally {
    core.free();
  }
});

test("values outside a field's rules are refused and change nothing", async () => {
  const { core, doc, saved } = await open();
  try {
    const before = saved();
    await expect(doc.fields.label.set("😀😀😀")).rejects.toThrow("out_of_range");
    await expect(doc.fields.ratio.set(2)).rejects.toThrow("out_of_range");
    await expect(doc.fields.rating.set(1.5)).rejects.toThrow("type_mismatch");
    await expect(doc.fields.currency.set("GBP" as any)).rejects.toThrow("type_mismatch");
    await expect(doc.fields.photo.id.set("x")).rejects.toThrow("path_not_found");
    expect(saved()).toEqual(before);
  } finally {
    core.free();
  }
});

// Failure: a drag wrote history on every frame, or a preview outlived its write.
test("previews stay local until set or flush and never enter the core early", async () => {
  const { core, doc, saved } = await open();
  try {
    doc.fields.ratio.preview(0.9);
    doc.fields.rows.item("r1").amount.preview(500);
    expect(doc.current.ratio).toBe(0.9);
    expect(doc.current.rows[0]!.amount).toBe(500);
    expect(saved().ratio).toBe(0.5);
    expect(doc.status).toBe("pending");
    await doc.fields.ratio.set(0.7);
    expect(doc.current.ratio).toBe(0.7);
    expect(doc.current.rows[0]!.amount).toBe(500);
    await doc.flush();
    expect(saved().rows[0].amount).toBe(500);
    expect(doc.status).toBe("saved");
    // A preview on a row that is removed disappears with it.
    doc.fields.rows.item("r1").amount.preview(1);
    await doc.fields.rows.remove("r1");
    expect(doc.current.rows).toEqual([]);
    expect(doc.status).toBe("pending");
    await doc.flush();
    expect(doc.status).toBe("saved");
  } finally {
    core.free();
  }
});

class Control extends EventTarget {
  value = "";
  checked = false;
  disabled = false;
  constructor(readonly type: string) {
    super();
  }
  enter(value: string, event: "input" | "change") {
    this.value = value;
    this.dispatchEvent(new Event(event));
  }
}
const control = (type: string) => new Control(type) as Control & HTMLInputElement;

test("bindValue writes each scalar from its control, coalescing text", async () => {
  const { core, doc, saved } = await open();
  const range = control("range"), rating = control("number"), select = control("select-one");
  const label = control("text"), memo = control("text");
  const bindings = [
    doc.bindValue(range, doc.fields.ratio),
    doc.bindValue(rating, doc.fields.rating),
    doc.bindValue(select, doc.fields.currency),
    doc.bindValue(label, doc.fields.label),
    doc.bindValue(memo, doc.fields.memo),
  ];
  try {
    expect([range.value, rating.value, select.value, label.value, memo.value]).toEqual(["0.5", "3", "CAD", "ab", ""]);
    range.enter("0.8", "input");
    expect(doc.current.ratio).toBe(0.8);
    expect(saved().ratio).toBe(0.5);
    range.dispatchEvent(new Event("change"));
    rating.enter("4", "change");
    select.enter("USD", "change");
    for (const text of ["a", "ab", "abc", "abcd"]) label.enter(text, "input");
    memo.enter("note", "input");
    await doc.flush();
    expect(saved()).toMatchObject({ ratio: 0.8, rating: 4, currency: "USD", label: "abcd", memo: "note" });
    // Empty clears an optional; an invalid value reverts a required field.
    memo.enter("", "input");
    rating.enter("", "change");
    await doc.flush();
    expect("memo" in saved()).toBe(false);
    expect(saved().rating).toBe(4);
    expect(rating.value).toBe("4");
    // A value the core refuses shows the document's value again.
    label.enter("toolong", "input");
    await doc.flush().catch(() => {});
    expect(saved().label).toBe("abcd");
    expect(label.value).toBe("abcd");
  } finally {
    bindings.forEach((binding) => binding.destroy());
    core.free();
  }
});
