// Projection recovery reads the real WASM owner; expected values and bounded promise
// settlement are independent of delivery order and never depend on mutation replay.
import { expect, test } from "bun:test";
import { Store, recoveryPolicy } from "../src/owner/store";
import { DocumentError } from "@hitslop/document/internal";
import { defineDocument, s } from "@hitslop/document";
import * as wasm from "../../../generated/core/wasm/hitslop_core_wasm.js";
wasm.initSync({ module: await Bun.file(new URL("../../../generated/core/wasm/hitslop_core_wasm_bg.wasm", import.meta.url)).bytes() });
const policy = { ...recoveryPolicy, stallMS: 5, deadlineMS: 100, retryMS: 1, maxRetryMS: 5, maxItems: 2 };
function fixture() {
  const definition = defineDocument({ count: s.counter() });
  const core = wasm.WasmDocument.create(JSON.stringify(definition.descriptor), JSON.stringify({ count: 0 }));
  const open = async () => JSON.parse(core.snapshot());
  const edit = () => core.applyBatch(JSON.stringify({ intents: [{ type: "increment", path: ["count"], by: 1 }] }));
  return { core, open, edit };
}

test("recovery retries transient reads and preserves the accepted counter increment", async () => {
  const { core, open, edit } = fixture();
  let attempts = 0;
  const store = new Store(async () => { if (++attempts < 3) throw Error("temporarily unavailable"); return open(); }, () => {}, policy);
  try {
    store.load(await open());
    edit();
    await store.reached(1);
    expect(store.state.value).toEqual({ count: 1 });
    expect(JSON.parse(core.snapshot()).value).toEqual({ count: 1 });
  } finally { core.free(); }
});

for (const terminal of [false, true]) test(`recovery failure settles and explicit recovery unblocks edits: ${terminal}`, async () => {
  const { core, open, edit } = fixture();
  let recovered = false;
  const store = new Store(async () => {
    if (recovered) return open();
    if (terminal) throw new DocumentError("owner_invalidated", "owner unavailable");
    return new Promise<never>(() => {});
  }, () => {}, policy);
  try {
    store.load(await open()); edit();
    await expect(store.reached(1)).rejects.toThrow(terminal ? "owner unavailable" : "timed out");
    expect(() => store.assertWritable()).toThrow();
    expect(JSON.parse(core.snapshot()).value).toEqual({ count: 1 });
    recovered = true;
    await store.resync();
    store.assertWritable();
    expect(store.state.value).toEqual({ count: 1 });
  } finally { core.free(); }
});

test("overflow during snapshot loading requires another snapshot", async () => {
  const { core, open, edit } = fixture();
  let release!: () => void;
  const gate = new Promise<void>(resolve => release = resolve);
  let first = true;
  const store = new Store(async () => {
    const state = await open();
    if (first) { first = false; await gate; }
    return state;
  }, () => {}, policy);
  try {
    store.load(await open());
    const recovering = store.resync();
    for (let i = 0; i < 4; i++) {
      const reply = edit();
      store.publish([{ type: "publication", publication: JSON.parse(reply.publication!) }]);
    }
    release(); await recovering;
    expect(store.state.value).toEqual({ count: 4 });
    expect(store.state.sequence).toBe(4);
  } finally { release(); core.free(); }
});

// Failure: a text change misplaced across surrogate pairs, or issues dropped when a
// publication omits them. Oracle: literal strings and the owner's fresh snapshot.
test("text publications apply in code points and issues persist until they change", async () => {
  const definition = defineDocument({ title: s.text() });
  const core = wasm.WasmDocument.create(JSON.stringify(definition.descriptor), JSON.stringify({ title: "a😀b" }));
  const open = async () => JSON.parse(core.snapshot());
  const store = new Store(open, () => {}, policy);
  try {
    store.load(await open());
    const issues = [{ code: "unknown_field" as const, path: ["extra"] }];
    store.publish([{ type: "publication", publication: { previous: 0, sequence: 1, version: "", ops: [], issues } }]);
    const edit = core.editText(JSON.stringify({ base: core.version(), path: ["title"], from: "a😀b", to: "a😀xb", selectionStart: 4, selectionEnd: 4 }));
    const publication = JSON.parse(edit.publication!);
    expect(publication.ops).toEqual([{ type: "text", path: ["title"], delta: [{ retain: 2 }, { insert: "x" }] }]);
    store.publish([{ type: "publication", publication: { ...publication, previous: 1, sequence: 2 } }]);
    expect(store.state.value).toEqual({ title: "a😀xb" });
    expect(store.state.issues).toEqual(issues);
    // A change that does not fit the field forces a fresh snapshot instead.
    store.publish([{ type: "publication", publication: { previous: 2, sequence: 3, version: "", ops: [{ type: "text", path: ["title"], delta: [{ retain: 9 }] }] } }]);
    await store.reached(1);
    expect(store.state.value).toEqual(JSON.parse(core.snapshot()).value);
  } finally { core.free(); }
});

// Failure: when a later publication in a delivered batch could not apply, the earlier ones
// were installed silently, so views showed stale values until a resync completed.
test("publications applied before one that fails are announced at once", async () => {
  const { core, open, edit } = fixture();
  const announced: unknown[] = [];
  const store = new Store(() => new Promise<never>(() => {}), (changes) => announced.push(changes), policy);
  try {
    store.load(await open());
    announced.length = 0;
    const publication = JSON.parse(edit().publication!);
    const broken = { ...publication, previous: publication.sequence, sequence: publication.sequence + 1,
      ops: [{ type: "text", path: ["count"], delta: [{ retain: 1 }] }] };
    store.publish([
      { type: "publication", publication },
      { type: "publication", publication: broken },
    ]);
    expect(store.state.value).toEqual({ count: 1 });
    expect(announced).toEqual([{ paths: new Set(['["count"]']), removed: [] }]);
  } finally { core.free(); }
});
