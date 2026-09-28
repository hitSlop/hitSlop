// Guards persisted update ancestry, duplicate delivery, move/edit identity, preserve-and-flag
// merges and the outbound local-update stream.
import { expect, test } from "bun:test";
import { LoroDoc, type LoroMap, type LoroMovableList } from "loro-crdt";
import { Document } from "../src/document";
import { defineDocument, s } from "../src/schema";
import { MemoryStore } from "../src/memory";
import { copyStore } from "./helpers";
const schema = defineDocument({
  title: s.text(),
  tasks: s.list(s.object({ text: s.text(), done: s.boolean() })),
});
const initial = {
  title: "abc",
  tasks: [
    { text: "first", done: false },
    { text: "second", done: false },
    { text: "third", done: false },
  ],
};
async function pair() {
  const ia = new MemoryStore(),
    a = await Document.open(schema, ia, initial),
    ib = new MemoryStore();
  await copyStore(ia, ib);
  const b = await Document.open(schema, ib, initial);
  return { a, b, ia, ib };
}
function exchange(a: Document<any>, b: Document<any>) {
  const aa = a.exportUpdates(b.version()),
    bb = b.exportUpdates(a.version());
  a.importUpdates(bb);
  b.importUpdates(aa);
  a.importUpdates(bb);
  b.importUpdates(aa);
  expect(a.current).toEqual(b.current);
}
test("offline text ancestry, duplicate delivery and reopen converge", async () => {
  const { a, b, ia, ib } = await pair();
  for (const title of ["abcX", "abcXY", "abcXYZ"]) a.fields.title.replace(title);
  b.fields.title.replace("Qabc");
  b.fields.tasks.item(b.current.tasks[0]!.$id).done.set(true);
  exchange(a, b);
  expect(a.current.title).toBe("QabcXYZ");
  expect(a.current.tasks[0]!.done).toBe(true);
  await a.close();
  await b.close();
  const ar = await Document.open(schema, ia, initial),
    br = await Document.open(schema, ib, initial);
  exchange(ar, br);
  expect(ar.current.title).toBe("QabcXYZ");
  await ar.close();
  await br.close();
});
test("concurrent move/edit preserves identity and both edits", async () => {
  const { a, b } = await pair(),
    id = a.current.tasks[0]!.$id;
  a.fields.tasks.move(id, { after: a.current.tasks[2]!.$id });
  b.fields.tasks.item(id).text.replace("Edited");
  exchange(a, b);
  expect(a.current.tasks[2]!.$id).toBe(id);
  expect(a.current.tasks[2]!.text).toBe("Edited");
  await a.close();
  await b.close();
});
test("delete/edit convergence does not resurrect a row", async () => {
  const { a, b } = await pair(),
    id = a.current.tasks[0]!.$id;
  a.fields.tasks.remove(id);
  b.fields.tasks.item(id).text.replace("Edited");
  exchange(a, b);
  expect(a.current.tasks.some((row) => row.$id === id)).toBe(false);
  await a.close();
  await b.close();
});
test("out-of-order imports are rejected until dependencies arrive", async () => {
  const { a, b } = await pair(),
    before = a.version();
  a.fields.title.replace("abc1");
  const first = a.exportUpdates(before),
    mid = a.version();
  a.fields.title.replace("abc12");
  const second = a.exportUpdates(mid);
  expect(() => b.importUpdates(second)).toThrow("dependencies");
  expect(b.current.title).toBe("abc");
  b.importUpdates(first);
  b.importUpdates(second);
  expect(b.current.title).toBe("abc12");
  await a.close();
  await b.close();
});
test("remote semantic anomalies are accepted, converge, are flagged and are never repaired", async () => {
  for (const [mutate, issue] of [
    [
      (data: LoroMap) => ((data.get("tasks") as LoroMovableList).get(0) as LoroMap).set("done", "invalid"),
      { kind: "invalid", detail: "Expected boolean" },
    ],
    [(data: LoroMap) => data.set("unknown", true), { kind: "unknown-field", detail: "Unknown stored field: unknown" }],
    [(data: LoroMap) => data.set("title", "wrong container"), { kind: "invalid", detail: "Expected LoroText" }],
  ] as const) {
    const { a, b } = await pair();
    const fork = new LoroDoc();
    fork.import(b.exportSnapshot());
    mutate(fork.getMap("data"));
    fork.commit();
    const bytes = fork.export({ mode: "update", from: b.version() });
    a.importUpdates(bytes);
    b.importUpdates(bytes);
    expect(a.current).toEqual(b.current);
    expect(a.issues).toEqual(b.issues);
    expect(a.issues).toMatchObject([issue]);
    // Reading issues and state writes nothing: both peers still export the fork's version.
    expect(a.version().encode()).toEqual(fork.version().encode());
    fork.free();
    await a.close();
    await b.close();
  }
});
test("staged batch merges alongside another peer's text edit", async () => {
  const { a, b } = await pair();
  a.change((tx) => {
    tx.fields.title.replace("abcXYZ");
    tx.fields.tasks.item(a.current.tasks[0]!.$id).done.set(true);
  });
  b.fields.title.replace("Qabc");
  exchange(a, b);
  expect(a.current.title).toBe("QabcXYZ");
  expect(a.current.tasks[0]!.done).toBe(true);
  await a.close();
  await b.close();
});
test("counter increments that merge past the finite range keep both peers open and flag the value", async () => {
  const counted = defineDocument({ total: s.counter() });
  const ia = new MemoryStore(),
    a = await Document.open(counted, ia, { total: 0 }),
    ib = new MemoryStore();
  await copyStore(ia, ib);
  const b = await Document.open(counted, ib, { total: 0 });
  a.fields.total.increment(1e308);
  b.fields.total.increment(1e308);
  const aa = a.exportUpdates(b.version()),
    bb = b.exportUpdates(a.version());
  a.importUpdates(bb);
  b.importUpdates(aa);
  expect(a.version().encode()).toEqual(b.version().encode());
  expect(a.current.total).toBeNull();
  expect(a.issues).toEqual([{ path: ["total"], kind: "invalid", detail: "Expected finite number" }]);
  expect(() => a.fields.total.increment(1)).toThrow("finite");
  await a.close();
  await b.close();
});
test("every accepted local edit is emitted once for synchronization; remote imports are not", async () => {
  const { a, b } = await pair();
  const sent: Uint8Array[] = [];
  const stop = a.onLocalUpdate((bytes) => sent.push(bytes));
  const received: Uint8Array[] = [];
  const stopB = b.onLocalUpdate((bytes) => received.push(bytes));
  a.fields.title.replace("direct");
  a.change((tx) => tx.fields.tasks.item(a.current.tasks[0]!.$id).done.set(true));
  a.applyAll([{ type: "set", path: ["tasks", { id: a.current.tasks[1]!.$id }, "done"], value: true }], { origin: "cli" });
  a.importJSON({ ...a.current, title: "imported" });
  expect(sent).toHaveLength(4);
  for (const bytes of sent) b.importUpdates(bytes);
  expect(received).toHaveLength(0);
  expect(b.current).toEqual(a.current);
  stop();
  stopB();
  await a.close();
  await b.close();
});

// Incremental patching and a full read must apply one anomaly policy; otherwise the same
// stored bytes read differently before and after reopening, and peers cannot agree.
test("incremental and full projection agree for every anomaly position", async () => {
  const shaped = defineDocument({
    amount: s.number(),
    values: s.record(s.number()),
    cells: s.record(s.object({ raw: s.string() })),
    tags: s.list(s.string()),
    rows: s.list(s.object({ name: s.text(), done: s.boolean() })),
    outline: s.tree(s.object({ label: s.text() })),
    cover: s.optional(s.object({ caption: s.string() })),
    total: s.counter(),
  });
  const initial = {
    amount: 1,
    values: { ok: 1 },
    cells: { A1: { raw: "1" } },
    tags: ["t"],
    rows: [{ name: "r", done: false }],
    outline: [{ label: "root" }],
    total: 0,
  };
  const mutations: [string, (data: LoroMap) => void][] = [
    ["root scalar", (d) => d.set("amount", "nope")],
    ["record scalar entry", (d) => (d.get("values") as LoroMap).set("bad", "nope")],
    ["record object entry", (d) => (d.get("cells") as LoroMap).set("B2", 7)],
    ["record key", (d) => (d.get("values") as LoroMap).set("__proto__", 1)],
    ["list item", (d) => (d.get("tags") as LoroMovableList).insert(0, 5)],
    ["row field", (d) => ((d.get("rows") as LoroMovableList).get(0) as LoroMap).set("done", "yes")],
    ["row identity", (d) => ((d.get("rows") as LoroMovableList).get(0) as LoroMap).delete("$id")],
    ["wrong container", (d) => d.set("cover", "not an object")],
    ["unknown field", (d) => d.set("extra", 1)],
  ];
  for (const [name, mutate] of mutations) {
    const store = new MemoryStore();
    const doc = await Document.open(shaped, store, initial);
    const fork = new LoroDoc();
    fork.import(doc.exportSnapshot());
    mutate(fork.getMap("data"));
    fork.commit();
    doc.importUpdates(fork.export({ mode: "update", from: doc.version() }));
    const incremental = doc.current;
    const issues = doc.issues;
    await doc.close();
    const reopened = await Document.open(shaped, store, initial);
    expect([name, reopened.current]).toEqual([name, incremental]);
    expect([name, reopened.issues]).toEqual([name, issues]);
    await reopened.close();
    fork.free();
  }
});
