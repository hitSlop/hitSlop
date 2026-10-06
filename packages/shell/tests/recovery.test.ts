// Projection recovery reads the real WASM owner; expected values and bounded promise
// settlement are independent of delivery order and never depend on mutation replay.
import { afterEach, beforeEach, expect, jest, test } from "bun:test";
import { PushLimits } from "@hitslop/schema/constants";
import { Store } from "../src/owner/store";
import { DocumentError } from "@hitslop/document/internal";
import { defineDocument, s } from "@hitslop/document";
import * as wasm from "../../../generated/core/wasm/hitslop_core_wasm.js";
wasm.initSync({ module: await Bun.file(new URL("../../../generated/core/wasm/hitslop_core_wasm_bg.wasm", import.meta.url)).bytes() });
// The store's stall, retry and deadline timers run on fake time, advanced by `elapsing`.
beforeEach(() => jest.useFakeTimers());
afterEach(() => jest.useRealTimers());
/** Settles `work`, advancing fake time a step at a time and letting promises run between. */
async function elapsing<T>(work: Promise<T>): Promise<T> {
  let settled = false;
  work.then(() => (settled = true), () => (settled = true));
  for (let step = 0; step < 1000 && !settled; step++) {
    for (let turn = 0; turn < 20; turn++) await null;
    jest.advanceTimersByTime(50);
  }
  return work;
}
function fixture() {
  const definition = defineDocument({ count: s.counter() });
  const core = wasm.WasmDocument.create(JSON.stringify(definition.descriptor), JSON.stringify({ count: 0 }));
  const open = async () => JSON.parse(core.state());
  const edit = () => core.applyBatch(JSON.stringify({ intents: [{ type: "increment", path: ["count"], by: 1 }] }));
  return { core, open, edit };
}

test("theme-only publications share ordering and recover from a missing theme change", async () => {
  const definition = defineDocument({ title: s.text() });
  const core = wasm.WasmDocument.create(JSON.stringify(definition.descriptor),
    JSON.stringify({ title: "Title" }), "theme-test", JSON.stringify({ accent: "#112233" }));
  const open = async () => JSON.parse(core.state());
  const seen: string[] = [];
  const store = new Store(open, () => seen.push(store.state.theme.accent!));
  try {
    store.load(await open());
    const value = store.state.value;
    const first = JSON.parse(core.themeSet(JSON.stringify({ accent: "#445566" })).publication!);
    expect(first.ops).toEqual([]);
    store.publish([{ type: "publication", publication: first }]);
    await store.reached(first.sequence);
    expect(store.state.value).toBe(value);
    expect(store.state.theme).toEqual({ accent: "#445566" });
    // Theme omitted from a later content publication retains the accepted palette.
    const edit = core.applyBatch(JSON.stringify({ intents: [{ type: "set", path: ["title"], value: "Changed" }] }));
    store.publish([{ type: "publication", publication: JSON.parse(edit.publication!) }]);
    expect(store.state.theme).toEqual({ accent: "#445566" });
    // Lose one theme delivery: the next publication detects the gap and reads one frame.
    core.themeSet(JSON.stringify({ accent: "#778899" }));
    const latest = JSON.parse(core.themeSet(JSON.stringify({ accent: "#aabbcc" })).publication!);
    store.publish([{ type: "publication", publication: latest }]);
    await elapsing(store.reached(latest.sequence));
    expect(store.state.theme).toEqual({ accent: "#aabbcc" });
    expect(store.state.value).toEqual({ title: "Changed" });
    store.publish([{ type: "publication", publication: first }]);
    expect(store.state.theme).toEqual({ accent: "#aabbcc" });
    expect(seen).toEqual(["#112233", "#445566", "#445566", "#aabbcc"]);
  } finally { core.free(); }
});

test("recovery retries transient reads and preserves the accepted counter increment", async () => {
  const { core, open, edit } = fixture();
  let attempts = 0;
  const store = new Store(async () => { if (++attempts < 3) throw Error("temporarily unavailable"); return open(); }, () => {});
  try {
    store.load(await open());
    edit();
    await elapsing(store.reached(1));
    expect(store.state.value).toEqual({ count: 1 });
    expect(JSON.parse(core.state()).value).toEqual({ count: 1 });
  } finally { core.free(); }
});

for (const terminal of [false, true]) test(`recovery failure settles and explicit recovery unblocks edits: ${terminal}`, async () => {
  const { core, open, edit } = fixture();
  let recovered = false;
  const store = new Store(async () => {
    if (recovered) return open();
    if (terminal) throw new DocumentError("owner_invalidated", "owner unavailable");
    return new Promise<never>(() => {});
  }, () => {});
  try {
    store.load(await open()); edit();
    await expect(elapsing(store.reached(1))).rejects.toThrow(terminal ? "owner unavailable" : "timed out");
    expect(() => store.assertWritable()).toThrow();
    expect(JSON.parse(core.state()).value).toEqual({ count: 1 });
    recovered = true;
    await elapsing(store.resync());
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
  }, () => {});
  try {
    store.load(await open());
    const recovering = store.resync();
    // One past the buffer's bound overflows it while the snapshot loads.
    for (let i = 0; i < PushLimits.items + 1; i++) {
      const reply = edit();
      store.publish([{ type: "publication", publication: JSON.parse(reply.publication!) }]);
    }
    release(); await elapsing(recovering);
    expect(store.state.value).toEqual({ count: PushLimits.items + 1 });
    expect(store.state.sequence).toBe(PushLimits.items + 1);
  } finally { release(); core.free(); }
});

// Failure: a text change misplaced across surrogate pairs. Oracle: literal strings and
// the owner's fresh snapshot.
test("text publications apply in code points", async () => {
  const definition = defineDocument({ title: s.text() });
  const core = wasm.WasmDocument.create(JSON.stringify(definition.descriptor), JSON.stringify({ title: "a😀b" }));
  const open = async () => JSON.parse(core.state());
  const store = new Store(open, () => {});
  try {
    store.load(await open());
    const edit = core.applyBatch(JSON.stringify({ base: JSON.parse(core.state()).version, intents: [{ type: "set", path: ["title"], value: "a😀xb", from: "a😀b", selection: { start: 4, end: 4 } }] }));
    const publication = JSON.parse(edit.publication!);
    expect(publication.ops).toEqual([{ type: "text", path: ["title"], delta: [{ retain: 2 }, { insert: "x" }] }]);
    store.publish([{ type: "publication", publication }]);
    expect(store.state.value).toEqual({ title: "a😀xb" });
    // A change that does not fit the field forces a fresh snapshot instead.
    store.publish([{ type: "publication", publication: { previous: 1, sequence: 2, version: "", ops: [{ type: "text", path: ["title"], delta: [{ retain: 9 }] }] } }]);
    await elapsing(store.reached(1));
    expect(store.state.value).toEqual(JSON.parse(core.state()).value);
  } finally { core.free(); }
});

// Failure: when a later publication in a delivered batch could not apply, the earlier ones
// were installed silently, so views showed stale values until a resync completed.
test("publications applied before one that fails are announced at once", async () => {
  const { core, open, edit } = fixture();
  const announced: unknown[] = [];
  const store = new Store(() => new Promise<never>(() => {}), (changes) => announced.push(changes));
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
