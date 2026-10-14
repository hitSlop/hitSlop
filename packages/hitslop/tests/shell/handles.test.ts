// Handle member names against authored field names, through the SDK and the real WASM
// core. Failure: a handle member that shadows an authored field (a field named `set`
// could not be edited, or its object's handle could not be built), or a member added
// without the `$` that keeps it clear of every field name. Oracle: the handles' own
// property names and the core's saved state.
import { expect, test } from "bun:test";
import { OwnerDocument } from "../../src/shell/owner/document";
import { wasmTransport } from "./wasm-transport";
import { defineDocument, s } from "hitslop";
const wasm = await import(new URL("../../../../generated/core/wasm/hitslop_core_wasm.js", import.meta.url).href);
wasm.initSync({
  module: await Bun.file(new URL("../../../../generated/core/wasm/hitslop_core_wasm_bg.wasm", import.meta.url)).bytes(),
});

// Fields named like the members a handle could have, which are the author's to name.
const authored = ["clear", "node", "path", "set"];
const fields = () => ({ set: s.string(), clear: s.string(), path: s.string(), node: s.string() });
const values = (prefix: string) => ({ set: `${prefix}s`, clear: `${prefix}c`, path: `${prefix}p`, node: `${prefix}n` });
const definition = defineDocument({
  group: s.object(fields()),
  photo: s.optional(s.object(fields())),
  rows: s.list(s.object(fields())),
  cells: s.record(s.object(fields())),
}, { initial: { group: values("g") } });

async function open() {
  const core = wasm.WasmDocument.create(JSON.stringify(definition.descriptor), JSON.stringify({ group: values("g"), rows: [], cells: {} }));
  const doc = await OwnerDocument.open(definition, wasmTransport(core), () => {});
  const saved = () => JSON.parse(core.state()).value;
  return { core, doc, saved };
}
/** Every own property name, enumerable or not. */
const names = (handle: object) => Reflect.ownKeys(handle).filter((key): key is string => typeof key === "string");
/** What a handle carries beyond its authored fields: all of it must start with `$`. */
const extras = (handle: object) => names(handle).filter((name) => !authored.includes(name));

// The rule behind the surface: a handle that also carries authored fields adds members
// only with a `$`, which no field name can start with. Handles without fields (scalars,
// text, lists, records, counters) have no authored names to meet, so they keep plain names.
test("an object handle's own names are its fields plus members that start with $", async () => {
  const { core, doc } = await open();
  try {
    const { id } = await doc.fields.rows.insert(values("r"));
    await doc.fields.cells.put("A1", values("c"));
    const handles = {
      group: doc.fields.group,
      photo: doc.fields.photo,
      row: doc.fields.rows.item(id),
      entry: doc.fields.cells.entry("A1"),
    };
    for (const [kind, handle] of Object.entries(handles)) {
      expect(names(handle).filter((name) => !name.startsWith("$")).sort(), kind).toEqual(authored);
      for (const name of extras(handle)) expect(name, kind).toMatch(/^\$/);
    }
    // Only the optional object has members of its own; an object is its fields.
    expect(extras(handles.group)).toEqual([]);
    expect(extras(handles.photo).sort()).toEqual(["$clear", "$set"]);
    // A change() transaction builds the same handles.
    const inside = await doc.change((tx) => ({
      group: names(tx.fields.group).sort(),
      photo: extras(tx.fields.photo).sort(),
    }));
    expect(inside).toEqual({ group: authored, photo: ["$clear", "$set"] });
  } finally {
    core.free();
  }
});

test("an optional object keeps fields named set and clear; $set and $clear replace and remove it", async () => {
  const { core, doc, saved } = await open();
  try {
    // `photo.set` is the field, and the object is absent, so it has nothing to write to.
    expect(typeof doc.fields.photo.set.set).toBe("function");
    await expect(doc.fields.photo.set.set("x")).rejects.toThrow("path_not_found");
    await doc.fields.photo.$set(values("a"));
    await doc.fields.photo.set.set("edited set");
    await doc.fields.photo.clear.set("edited clear");
    expect(saved().photo).toEqual({ set: "edited set", clear: "edited clear", path: "ap", node: "an" });
    expect(JSON.parse(JSON.stringify(doc.current.photo))).toEqual(saved().photo);
    // Replacing it with a complete value writes over the object that is there.
    await doc.fields.photo.$set(values("b"));
    expect(saved().photo).toEqual(values("b"));
    await doc.fields.photo.$clear();
    expect("photo" in saved()).toBe(false);
    expect(doc.current.photo).toBeUndefined();
    // Inside a change() the same members collect the same writes.
    await doc.change((tx) => {
      tx.fields.photo.$set(values("t"));
      tx.fields.photo.node.set("edited node");
    });
    expect(saved().photo).toEqual({ ...values("t"), node: "edited node" });
    await doc.change((tx) => tx.fields.photo.$clear());
    expect("photo" in saved()).toBe(false);
    // An object that is not optional has no members: `group.set` is only its field.
    await doc.fields.group.set.set("still a field");
    expect(saved().group).toEqual({ ...values("g"), set: "still a field" });
  } finally {
    core.free();
  }
});
