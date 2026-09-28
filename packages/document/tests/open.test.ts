// Guards preserve-and-flag reads of anomalous stored state, undecodable-state rejection,
// and host byte/envelope decoding.
import { describe, test, expect } from "bun:test";
import { LoroDoc, LoroMap, LoroMovableList, LoroText } from "loro-crdt";
import { defineDocument, fromDescriptor, s, schemaKey } from "../src/schema";
import { Document } from "../src/document";
import { MemoryStore } from "../src/memory";
import { derivedID } from "../src/identity";
import { HostStore, base64 } from "../src/bridge";

const schema = defineDocument({
  title: s.text(),
  tasks: s.list(s.object({ text: s.text(), done: s.boolean() })),
});

const initial = { title: "List", tasks: [] };

async function openStored(build: (data: LoroMap) => void) {
  const engine = new LoroDoc();
  build(engine.getMap("data"));
  engine.commit();
  const store = new MemoryStore();
  await store.checkpoint("0", engine.export({ mode: "snapshot" }), schemaKey(schema.descriptor));
  const stored = await store.load();
  return { doc: await Document.open(schema, store, initial), store, stored };
}

describe("opening stored documents", () => {
  test("accepts a well-formed checkpoint", async () => {
    const { doc } = await openStored((data) => {
      data.setContainer("title", new LoroText()).update("Stored");
      const tasks = data.setContainer("tasks", new LoroMovableList());
      const row = tasks.insertContainer(0, new LoroMap());
      row.set("$id", "row1");
      row.setContainer("text", new LoroText()).update("One");
      row.set("done", true);
    });
    expect<unknown>(doc.current.tasks).toEqual([{ $id: "row1", text: "One", done: true }]);
    expect(doc.current.title).toBe("Stored");
    expect(doc.issues).toEqual([]);
    await doc.close();
  });
  // Merged or foreign state opens with explicit issues and deterministic fallbacks.
  // Opening and reading never write repairs; unusable containers refuse edits.
  const row = (tasks: LoroMovableList, id: unknown, done: unknown) => {
    const map = tasks.insertContainer(tasks.length, new LoroMap());
    if (id !== undefined) map.set("$id", id as string);
    map.setContainer("text", new LoroText()).update(String(id));
    map.set("done", done as boolean);
  };
  for (const [name, build, expected, issues] of [
    [
      "text stored as a string register",
      (data: LoroMap) => {
        data.set("title", "Stored");
        data.setContainer("tasks", new LoroMovableList());
      },
      { title: "", tasks: [] },
      [{ path: ["title"], kind: "invalid", detail: "Expected LoroText" }],
    ],
    [
      "unknown stored field",
      (data: LoroMap) => {
        data.setContainer("title", new LoroText()).update("Kept");
        data.setContainer("tasks", new LoroMovableList());
        data.set("extra", 1);
      },
      { title: "Kept", tasks: [] },
      [{ path: [], kind: "unknown-field", detail: "Unknown stored field: extra" }],
    ],
    [
      "invalid row scalar",
      (data: LoroMap) => {
        data.setContainer("title", new LoroText());
        row(data.setContainer("tasks", new LoroMovableList()), "a", "yes");
      },
      { title: "", tasks: [{ $id: "a", text: "a", done: false }] },
      [{ path: ["tasks", { id: "a" }, "done"], kind: "invalid", detail: "Expected boolean" }],
    ],
    [
      "missing field",
      (data: LoroMap) => {
        data.setContainer("title", new LoroText()).update("Only title");
      },
      { title: "Only title", tasks: [] },
      [{ path: ["tasks"], kind: "invalid", detail: "Missing value" }],
    ],
  ] as const) {
    test(`opens ${name} with issues and without repair`, async () => {
      const { doc, store, stored } = await openStored(build);
      expect<unknown>(doc.current).toEqual(expected);
      expect<unknown>(doc.issues).toEqual(issues);
      await doc.close();
      const after = await store.load();
      expect(after.generation).toBe(stored.generation);
      expect(after.checkpoint).toEqual(stored.checkpoint);
    });
  }
  test("edits beneath an unusable container are refused; other fields stay editable", async () => {
    const { doc } = await openStored((data) => {
      data.set("title", "Stored");
      data.setContainer("tasks", new LoroMovableList());
    });
    expect(() => doc.fields.title.replace("x")).toThrow("unusable");
    doc.fields.tasks.insert({ text: "New", done: false });
    expect(doc.current.tasks).toHaveLength(1);
    await doc.close();
  });
  // Apps key lists by $id and call at(row); broken identities must not break rendering.
  test("rows with missing, invalid or duplicate IDs get stable derived IDs, stay editable and are flagged", async () => {
    const { doc, store, stored } = await openStored((data) => {
      data.setContainer("title", new LoroText());
      const tasks = data.setContainer("tasks", new LoroMovableList());
      row(tasks, "a", true);
      row(tasks, undefined, false);
      row(tasks, "not an id!", false);
      row(tasks, "a", false);
    });
    const ids = doc.current.tasks.map((task) => task.$id);
    expect(ids[0]).toBe("a");
    for (const id of ids.slice(1)) expect(id).toMatch(/^x-[0-9a-z]{24}$/);
    expect(new Set(ids).size).toBe(4);
    expect(doc.current.tasks.map((task) => task.text)).toEqual(["a", "undefined", "not an id!", "a"]);
    expect<unknown>(doc.issues).toEqual([
      { path: ["tasks", { id: ids[1] }], kind: "identity", detail: "Missing row ID" },
      { path: ["tasks", { id: ids[2] }], kind: "identity", detail: "Invalid row ID" },
      { path: ["tasks", { id: ids[3] }], kind: "identity", detail: "Duplicate row ID" },
    ]);
    await doc.close();
    expect((await store.load()).checkpoint).toEqual(stored.checkpoint);
    // Derived IDs survive reopening and address the row; edits never write an ID.
    const reopened = await Document.open(schema, store, initial);
    expect(reopened.current.tasks.map((task) => task.$id)).toEqual(ids);
    reopened.at(reopened.current.tasks[3]!).done.set(true);
    reopened.fields.tasks.move(ids[3]!, { before: ids[0]! });
    expect(reopened.current.tasks[0]).toMatchObject({ $id: ids[3], done: true });
    expect(reopened.issues).toHaveLength(3);
    // Agents see the same anomalies through slop get --snapshot.
    expect(reopened.snapshotFor("Document.slop").issues).toEqual(reopened.issues);
    await reopened.close();
  });
  test("undecodable and dependency-incomplete state still fails to open", async () => {
    const store = new MemoryStore();
    await store.checkpoint("0", new Uint8Array([1, 2, 3]), schemaKey(schema.descriptor));
    await expect(Document.open(schema, store, initial)).rejects.toThrow();
  });
});

describe("schema identity", () => {
  // Stored documents open only under an identical key, so these rules are frozen:
  // keys sorted recursively, array order kept, no whitespace, JavaScript JSON numbers.
  test("keys are canonical JSON of the descriptor, independent of authoring order and normalization", () => {
    const descriptor = {
      root: {
        properties: {
          b: { max: 5, kind: "integer", min: 1 },
          a: { kind: "optional", inner: { kind: "enum", values: ["y", "x"] } },
          c: { kind: "number", min: 0.5 },
        },
        kind: "object",
      },
      format: 1,
    } as any;
    const golden =
      '{"format":1,"root":{"kind":"object","properties":{"a":{"inner":{"kind":"enum","values":["y","x"]},"kind":"optional"},"b":{"kind":"integer","max":5,"min":1},"c":{"kind":"number","min":0.5}}}}';
    expect(schemaKey(descriptor)).toBe(golden);
    expect(schemaKey(fromDescriptor(descriptor).descriptor)).toBe(golden);
  });
  test("derived row IDs use a frozen, deterministic hash of internal identity", () => {
    expect(derivedID("cid:7@12345:Map")).toBe("x-r01fjb7pch2ptdbhx3heg86d");
    expect(derivedID("2@99")).toBe("x-8ytk4r1fvcw0kgf6n48rg87f");
  });
});

describe("bridge", () => {
  test("base64 round-trips empty, odd and large payloads", () => {
    for (const size of [0, 1, 2, 3, 16_383, 16_384, 16_385, 100_001]) {
      const bytes = new Uint8Array(size).map((_, i) => (i * 31 + 7) & 255);
      const text = base64.encode(bytes);
      expect(text).toBe(Buffer.from(bytes).toString("base64"));
      expect(base64.decode(text)).toEqual(bytes);
    }
  });
  test("base64 falls back without native Uint8Array helpers", () => {
    const from = Object.getOwnPropertyDescriptor(Uint8Array, "fromBase64");
    const to = Object.getOwnPropertyDescriptor(Uint8Array.prototype, "toBase64");
    delete (Uint8Array as any).fromBase64;
    delete (Uint8Array.prototype as any).toBase64;
    try {
      const bytes = new Uint8Array(40_000).map((_, i) => (i * 13) & 255);
      const text = base64.encode(bytes);
      expect(text).toBe(Buffer.from(bytes).toString("base64"));
      expect(base64.decode(text)).toEqual(bytes);
    } finally {
      if (from) Object.defineProperty(Uint8Array, "fromBase64", from);
      if (to) Object.defineProperty(Uint8Array.prototype, "toBase64", to);
    }
  });
  test("HostStore decodes stored bytes from the host", async () => {
    const calls: unknown[] = [];
    (globalThis as any).webkit = {
      messageHandlers: {
        storage: {
          postMessage: async (args: unknown) => {
            calls.push(args);
            return {
              checkpoint: base64.encode(new Uint8Array([1, 2, 3])),
              updates: [base64.encode(new Uint8Array([4]))],
              generation: "1",
              schemaKey: "k",
            };
          },
        },
      },
    };
    try {
      const stored = await new HostStore().load();
      expect(stored.checkpoint).toEqual(new Uint8Array([1, 2, 3]));
      expect(stored.updates).toEqual([new Uint8Array([4])]);
      expect(calls).toEqual([{ method: "load" }]);
    } finally {
      delete (globalThis as any).webkit;
    }
  });
});
