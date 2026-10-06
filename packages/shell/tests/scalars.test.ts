// Scalar kinds through the SDK and the real WASM core. Failure: a handle that writes the
// wrong intent, a preview that leaks into history or outlives its write, or an assigned
// value that is committed wrongly. Oracle: literal snapshots and the core's state.
import { expect, test } from "bun:test";
import { OwnerDocument } from "../src/owner/document";
import { wasmTransport, type OwnerTransport } from "../src/owner/transport";
import { defineDocument, s } from "@hitslop/document";
const wasm = await import(new URL("../../../generated/core/wasm/hitslop_core_wasm.js", import.meta.url).href);
wasm.initSync({
  module: await Bun.file(new URL("../../../generated/core/wasm/hitslop_core_wasm_bg.wasm", import.meta.url)).bytes(),
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
  let reported = () => {};
  /** Resolves at the next report. */
  const nextReport = () => new Promise<void>((resolve) => (reported = resolve));
  const transport = wasmTransport(core) as OwnerTransport & Record<string, any>;
  const doc = await OwnerDocument.open(definition, transport, (error) => {
    errors.push(error);
    reported();
  });
  const saved = () => JSON.parse(core.state()).value;
  return { core, doc, errors, nextReport, saved, transport };
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
    await doc.fields.ratio.set(0.7);
    expect(doc.current.ratio).toBe(0.7);
    expect(doc.current.rows[0]!.amount).toBe(500);
    await doc.flush();
    expect(saved().rows[0].amount).toBe(500);
    // A preview on a row that is removed disappears with it.
    doc.fields.rows.item("r1").amount.preview(1);
    await doc.fields.rows.remove("r1");
    expect(doc.current.rows).toEqual([]);
    await doc.flush();
    expect(saved().rows).toEqual([]);
  } finally {
    core.free();
  }
});

/** Resolves on the first document notification where `ready` holds. */
const until = (doc: OwnerDocument<any>, ready: () => boolean) =>
  new Promise<void>((resolve) => {
    if (ready()) return resolve();
    const stop = doc.subscribe(() => {
      if (ready()) { stop(); resolve(); }
    });
  });

// Failure: a scalar write showed nothing until the owner replied, or a refused one
// stayed on screen. Oracle: the snapshot before acceptance, and the core's state after.
test("live scalar writes show at once, and a refused write reverts", async () => {
  const { core, doc, errors, saved } = await open();
  try {
    const accepted = doc.fields.rating.set(4);
    expect(doc.current.rating).toBe(4);
    expect(saved().rating).toBe(3);
    await accepted;
    expect(saved().rating).toBe(4);
    const refused = doc.fields.rating.set(9);
    expect(doc.current.rating).toBe(9);
    await expect(refused).rejects.toThrow();
    expect(doc.current.rating).toBe(4);
    await doc.fields.memo.set("note");
    const cleared = doc.fields.memo.clear();
    expect(doc.current.memo).toBeUndefined();
    await cleared;
    expect("memo" in saved()).toBe(false);
    // The author caught the refusal, so nothing was reported.
    expect(errors).toEqual([]);
  } finally {
    core.free();
  }
});

// Failure: a bound control wrote each keystroke to history, could not clear an optional
// field, or left a refused value on screen. Oracle: literal snapshots, the core's state
// and the number of batches sent.
test("assigned values coalesce into one commit, clear optionals and revert refusals", async () => {
  const { core, doc, errors, nextReport, saved, transport } = await open();
  let sent = 0;
  const apply = transport.apply;
  transport.apply = (batch) => { sent++; return apply(batch); };
  try {
    for (const label of ["a", "ab", "abc", "abcd"]) doc.fields.label.value = label;
    expect(doc.fields.label.value).toBe("abcd");
    expect(doc.current.label).toBe("abcd");
    expect(saved().label).toBe("ab");
    await until(doc, () => saved().label === "abcd");
    await doc.flush();
    expect(sent).toBe(1);
    // An emptied number input assigns null: it clears an optional and is ignored otherwise.
    doc.fields.memo.value = "note";
    doc.fields.memo.value = null as never;
    doc.fields.rating.value = null as never;
    expect(doc.current.memo).toBeUndefined();
    expect(doc.current.rating).toBe(3);
    await doc.flush();
    expect("memo" in saved()).toBe(false);
    expect(saved().rating).toBe(3);
    // Nobody awaits an assignment, so a refusal is reported and the saved value returns.
    const report = nextReport();
    doc.fields.label.value = "toolong";
    expect(doc.current.label).toBe("toolong");
    await report;
    expect(doc.current.label).toBe("abcd");
    expect(saved().label).toBe("abcd");
    expect(errors).toHaveLength(1);
  } finally {
    core.free();
  }
});
