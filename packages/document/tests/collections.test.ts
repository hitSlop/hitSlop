// Records, scalar lists and optional text through the SDK and the real WASM core.
// Failure: a handle that writes the wrong path or intent, a preview that leaks into
// history, a record entry `doc.at` cannot resolve, or typing into an unset optional text
// that is lost. Oracle: literal snapshots and the core's saved state.
import { expect, test } from "bun:test";
import { OwnerDocument } from "../src/owner/document";
import { wasmTransport } from "../src/owner/transport";
import { defineDocument, s } from "../src/schema";
const wasm = await import(new URL("../../../generated/core/wasm/hitslop_core_wasm.js", import.meta.url).href);
wasm.initSync({
  module: await Bun.file(new URL("../../../generated/core/wasm/hitslop_core_wasm_bg.wasm", import.meta.url)).bytes(),
});

const definition = defineDocument({
  done: s.record(s.boolean()),
  cells: s.record(s.object({ input: s.string({ maxLength: 10 }), tint: s.optional(s.enum(["red", "blue"])) })),
  pages: s.record(s.object({ text: s.text() })),
  pixels: s.list(s.string()),
  presets: s.list(s.integer({ min: 40, max: 240 })),
  notes: s.optional(s.text()),
});
const initial = { done: {}, cells: {}, pages: {}, pixels: ["#fff", "#fff"], presets: [60, 90] };

async function open() {
  const core = wasm.WasmDocument.create(JSON.stringify(definition.descriptor), JSON.stringify(initial));
  const doc = await OwnerDocument.open(definition, wasmTransport(core), () => {});
  const saved = () => JSON.parse(core.snapshot()).value;
  return { core, doc, saved };
}

test("record handles put, edit, resolve and delete entries by key", async () => {
  const { core, doc, saved } = await open();
  try {
    await doc.fields.done.put("3:5", true);
    await doc.fields.cells.put("A1", { input: "hi" });
    await doc.fields.cells.entry("A1").tint.set("red");
    // A snapshot entry resolves to its handle, keyed by its record key.
    await doc.at(doc.current.cells["A1"]!).input.set("yo");
    await doc.change((tx) => {
      tx.fields.cells.put("B2", { input: "x" });
      tx.fields.done.delete("3:5");
    });
    expect(saved()).toEqual({
      done: {}, cells: { A1: { input: "yo", tint: "red" }, B2: { input: "x" } }, pages: {},
      pixels: ["#fff", "#fff"], presets: [60, 90],
    });
    expect(JSON.parse(JSON.stringify(doc.current))).toEqual(saved());
    await expect(doc.fields.cells.entry("Z9").input.set("x")).rejects.toThrow("path_not_found");
    await expect(doc.fields.done.put("__proto__", true)).rejects.toThrow("invalid_key");
    await doc.fields.pages.put("d1", { text: "a" });
    await expect(doc.fields.pages.put("d1", { text: "b" })).rejects.toThrow("exists");
    await doc.fields.pages.entry("d1").text.set("edited");
    expect(saved().pages).toEqual({ d1: { text: "edited" } });
  } finally {
    core.free();
  }
});

test("scalar list handles insert, set, remove and replace by index", async () => {
  const { core, doc, saved } = await open();
  try {
    await doc.fields.pixels.insert("#000", 1);
    await doc.fields.pixels.set(0, "#f00");
    await doc.fields.presets.insert(120);
    await doc.fields.presets.remove(0);
    expect(saved()).toMatchObject({ pixels: ["#f00", "#000", "#fff"], presets: [90, 120] });
    await doc.fields.presets.replace([60, 70, 90]);
    expect(saved().presets).toEqual([60, 70, 90]);
    await expect(doc.fields.presets.insert(300)).rejects.toThrow("out_of_range");
    await expect(doc.fields.pixels.set(9, "#000")).rejects.toThrow("path_not_found");
    expect(doc.current.pixels).toEqual(["#f00", "#000", "#fff"]);
  } finally {
    core.free();
  }
});

// Failure: a paint stroke wrote history per pixel, or a preview outlived its write.
test("a preview on a list element stays local until set or flush", async () => {
  const { core, doc, saved } = await open();
  try {
    doc.fields.pixels.preview(0, "#123");
    doc.fields.pixels.preview(1, "#456");
    expect(doc.current.pixels).toEqual(["#123", "#456"]);
    expect(saved().pixels).toEqual(["#fff", "#fff"]);
    await doc.fields.pixels.set(0, "#abc");
    expect(doc.current.pixels).toEqual(["#abc", "#456"]);
    await doc.flush();
    expect(saved().pixels).toEqual(["#abc", "#456"]);
  } finally {
    core.free();
  }
});

class Field extends EventTarget {
  value = "";
  selectionStart = 0;
  selectionEnd = 0;
  disabled = false;
  setSelectionRange(start: number, end: number) {
    this.selectionStart = start;
    this.selectionEnd = end;
  }
  blur() {}
  type(value: string) {
    this.value = value;
    this.setSelectionRange(value.length, value.length);
    this.dispatchEvent(new Event("input"));
  }
}

test("an unset optional text binds as empty and typing creates it", async () => {
  const { core, doc, saved } = await open();
  const field = new Field() as Field & HTMLInputElement;
  const binding = doc.bindText(field, doc.fields.notes);
  try {
    expect(field.value).toBe("");
    expect(field.disabled).toBe(false);
    field.type("First note");
    await doc.flush();
    expect(saved().notes).toBe("First note");
    field.type("First note, edited");
    await doc.flush();
    expect(saved().notes).toBe("First note, edited");
    await doc.fields.notes.clear();
    expect("notes" in saved()).toBe(false);
    expect(field.value).toBe("");
  } finally {
    binding.destroy();
    core.free();
  }
});
