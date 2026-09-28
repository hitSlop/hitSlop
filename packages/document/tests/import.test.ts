// Guards whole-document import identity, atomic rejection, references and rich-text preservation.
import { expect, test } from "bun:test";
import { Document } from "../src/document";
import { defineDocument, s } from "../src/schema";
import { MemoryStore } from "../src/memory";

const definition = defineDocument({
  title: s.text(),
  rows: s.list(s.object({ name: s.string(), link: s.string() })),
  notes: s.richtext({ bold: "after" }),
  count: s.counter(),
  tags: s.list(s.string()),
  extra: s.optional(s.object({ rows: s.list(s.object({ value: s.integer() })) })),
  tree: s.tree(s.object({ name: s.string() })),
  values: s.record(s.number()),
});
const initial = {
  title: "Old",
  rows: [
    { name: "A", link: "" },
    { name: "B", link: "" },
  ],
  notes: "",
  count: 1,
  tags: ["old"],
  tree: [{ name: "parent", children: [{ name: "child" }] }],
  values: { old: 1 },
};

test("import preserves rows, reparents surviving descendants, resolves cycles and reopens", async () => {
  const store = new MemoryStore();
  const doc = await Document.open(definition, store, initial);
  const a = doc.current.rows[0]!.$id,
    child = doc.current.tree[0]!.children[0]!.$id;
  doc.importJSON({
    title: "Today",
    rows: [
      { name: "New", link: { $ref: "/rows/1" } },
      { $id: a, name: "Updated A", link: { $ref: "/rows/0" } },
    ],
    notes: {
      text: "Hi 🌻",
      delta: [{ insert: "Hi " }, { insert: "🌻", attributes: { bold: true } }],
    },
    count: 7,
    tags: ["new"],
    extra: { rows: [{ value: 42 }] },
    tree: [{ $id: child, name: "surviving child" }],
    values: { next: 2 },
  });
  const saved = doc.current;
  expect(saved.rows.map((r) => r.name)).toEqual(["New", "Updated A"]);
  expect(saved.rows[1]!.$id).toBe(a);
  expect(saved.rows[0]!.link).toBe(a);
  expect(saved.rows[1]!.link).toBe(saved.rows[0]!.$id);
  expect(JSON.parse(JSON.stringify(saved.tree))).toEqual([
    { $id: child, name: "surviving child", children: [] },
  ]);
  expect(saved.extra!.rows[0]!.value).toBe(42);
  expect(saved.notes.delta).toEqual([
    { insert: "Hi " },
    { insert: "🌻", attributes: { bold: true } },
  ]);
  expect(saved.count).toBe(7);
  expect(Object.entries(saved.values)).toEqual([["next", 2]]);
  await doc.close();
  const reopened = await Document.open(definition, store, initial);
  expect(reopened.current).toEqual(saved);
  const version = reopened.version().encode();
  const generation = (await store.load()).generation;
  reopened.importJSON(JSON.parse(JSON.stringify(saved)));
  expect(reopened.version().encode()).toEqual(version);
  await reopened.flush();
  expect((await store.load()).generation).toBe(generation);
  await reopened.close();
});

test("bad imports reject without changing state, version or persisted updates", async () => {
  const store = new MemoryStore(),
    doc = await Document.open(definition, store, initial);
  const state = doc.current,
    version = doc.version().encode();
  const data = JSON.parse(JSON.stringify(state));
  for (const [bad, message] of [
    [{ ...data, count: "wrong" }, "/count: Expected finite number"],
    [{ ...data, rows: [{ name: "New", link: { $ref: "/missing" } }] }, "/rows/0/link: Reference"],
    [{ ...data, rows: [data.rows[0], data.rows[0]] }, "/rows/1/$id: Expected a unique"],
    [
      { ...data, notes: { text: "wrong", delta: [{ insert: "different" }] } },
      "/notes: Rich text and delta disagree",
    ],
    [
      { ...data, rows: [], tree: [{ $id: data.rows[0].$id, name: "wrong container" }] },
      "/tree/0/$id: Cannot reuse",
    ],
    [{ ...data, surprise: true }, "/surprise: Unknown field"],
    [{ ...data, title: "bad\uD800" }, "/title: Unpaired Unicode surrogate"],
    [
      { ...data, notes: { text: "x", delta: [{ insert: "x", attributes: { unknown: true } }] } },
      "/notes/delta/0: Unknown rich text mark",
    ],
  ] as const) {
    expect(() => doc.importJSON(bad)).toThrow(message);
    expect(doc.current).toEqual(state);
    expect(doc.version().encode()).toEqual(version);
  }
  await doc.flush();
  expect((await store.load()).updates).toHaveLength(0);
  await doc.close();
});

test("creation ignores source IDs, handles nested collections and escaped reference pointers", async () => {
  const schema = defineDocument({
    groups: s.record(
      s.object({
        rows: s.list(
          s.object({ label: s.string(), selected: s.boolean(), mode: s.enum(["a", "b"]) }),
        ),
      }),
    ),
    links: s.list(s.string()),
    link: s.optional(s.string()),
    counter: s.counter(),
  });
  const doc = await Document.open(schema, new MemoryStore(), {
    groups: { "a/b~": { rows: [{ label: "Old", selected: false, mode: "a" }] } },
    links: [],
    counter: 100,
  });
  const old = doc.current.groups["a/b~"]!.rows[0]!.$id;
  doc.importJSON(
    {
      groups: { "a/b~": { rows: [{ $id: old, label: "New", selected: true, mode: "b" }] } },
      links: [{ $ref: "/groups/a~1b~0/rows/0" }],
      link: { $ref: "/groups/a~1b~0/rows/0" },
      counter: 1,
    },
    { fresh: true },
  );
  const next = doc.current.groups["a/b~"]!.rows[0]!;
  expect(next.$id).not.toBe(old);
  expect(next.label).toBe("New");
  expect(next.selected).toBe(true);
  expect(next.mode).toBe("b");
  expect(doc.current.link).toBe(next.$id);
  expect(doc.current.links).toEqual([next.$id]);
  expect(doc.current.counter).toBe(1);
  await doc.close();
});

test("tree ancestry can reverse without losing identities; plain rich text removes marks", async () => {
  const doc = await Document.open(definition, new MemoryStore(), initial);
  const parent = doc.current.tree[0]!.$id,
    child = doc.current.tree[0]!.children[0]!.$id;
  doc.importJSON({
    ...doc.current,
    tree: [{ $id: child, name: "child", children: [{ $id: parent, name: "parent" }] }],
    notes: { text: "bold", delta: [{ insert: "bold", attributes: { bold: true } }] },
  });
  expect(doc.current.tree[0]!.$id).toBe(child);
  expect(doc.current.tree[0]!.children[0]!.$id).toBe(parent);
  doc.importJSON({ ...doc.current, notes: "bold" });
  expect(doc.current.notes.delta).toEqual([{ insert: "bold" }]);
  await doc.close();
});

test("post-allocation reference bounds reject before acceptance", async () => {
  const bounded = defineDocument({
    rows: s.list(s.object({ name: s.string() })),
    ref: s.string({ maxLength: 1 }),
  });
  const doc = await Document.open(bounded, new MemoryStore(), { rows: [], ref: "" });
  const version = doc.version().encode();
  expect(() => doc.importJSON({ rows: [{ name: "New" }], ref: { $ref: "/rows/0" } })).toThrow(
    "/ref: Expected at most 1",
  );
  expect(doc.current.rows).toEqual([]);
  expect(doc.version().encode()).toEqual(version);
  await doc.close();
});

test("counter targets that floating-point increments cannot represent reject atomically", async () => {
  const schema = defineDocument({ title: s.text(), counter: s.counter() });
  const doc = await Document.open(schema, new MemoryStore(), { title: "Keep", counter: 1e16 });
  const before = doc.version().encode();
  expect(() => doc.importJSON({ title: "Discard", counter: 1 })).toThrow(
    "/counter: Counter target exceeds available numeric precision",
  );
  expect(doc.current.title).toBe("Keep");
  expect(doc.current.counter).toBe(1e16);
  expect(doc.version().encode()).toEqual(before);
  await doc.close();
});

test("bulk text imports complete and preserve whole Unicode code points", async () => {
  const schema = defineDocument({ title: s.text() });
  const doc = await Document.open(schema, new MemoryStore(), { title: "Imported" });
  const large = "bulk ".repeat(230_000);
  doc.importJSON({ title: large });
  expect(doc.current.title === large).toBe(true);
  for (const title of ["a😀z", "a😁z", "a🨀z", "😀😁", "😁😁", "a🦊b", "ab"]) {
    doc.importJSON({ title });
    expect(doc.current.title).toBe(title);
  }
  await doc.close();
});

test("accepted counter targets survive batched persistence with large historical increments", async () => {
  const schema = defineDocument({ n: s.counter() }),
    store = new MemoryStore();
  const doc = await Document.open(schema, store, { n: 1e16 });
  doc.importJSON({ n: 100 });
  doc.importJSON({ n: 1 });
  expect(doc.current.n).toBe(1);
  await doc.close();
  const reopened = await Document.open(schema, store, { n: 0 });
  expect(reopened.current.n).toBe(1);
  await reopened.close();
});

test("row and tree IDs are application values that survive a JSON round trip into a new document", async () => {
  const source = await Document.open(definition, new MemoryStore(), initial);
  source.fields.rows.item(source.current.rows[0]!.$id).link.set(source.current.rows[1]!.$id);
  const ids = [...source.current.rows.map((r) => r.$id), source.current.tree[0]!.$id, source.current.tree[0]!.children[0]!.$id];
  // Application IDs are short random strings, not engine container or tree IDs.
  for (const id of ids) expect(id).toMatch(/^[0-9a-z]{26}$/);
  expect(new Set(ids).size).toBe(ids.length);
  const exported = JSON.parse(JSON.stringify({ ...source.current, notes: source.current.notes.text }));
  const copy = await Document.open(definition, new MemoryStore(), { ...initial, rows: [], tree: [] });
  copy.importJSON(exported);
  expect(copy.current.rows.map((r) => r.$id)).toEqual(ids.slice(0, 2));
  expect(copy.current.rows[0]!.link).toBe(ids[1]);
  expect(copy.current.tree[0]!.children[0]!.$id).toBe(ids[3]);
  copy.fields.rows.item(ids[1]!).name.set("Addressable");
  expect(copy.current.rows[1]!.name).toBe("Addressable");
  expect(() => copy.fields.rows.insert({ name: "Dup", link: "" }) && copy.execute({ type: "insert", path: ["rows"], value: { name: "x", link: "" }, id: ids[0] })).toThrow("already used");
  await source.close();
  await copy.close();
});
