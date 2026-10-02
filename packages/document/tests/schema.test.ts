// Guards the canonical schema key (the host refuses a document whose stored key differs)
// and the base64 codec used for attachment bytes on the host bridge.
import { describe, test, expect } from "bun:test";
import { fromDescriptor, schemaKey } from "../src/descriptor";
import { defineDocument, s } from "../src/schema";
import { base64 } from "../../shell/src/bridge";

describe("schema identity", () => {
  // Stored documents open only under an identical key, so these rules are frozen:
  // keys sorted recursively, array order kept, no whitespace, JavaScript JSON numbers.
  test("keys are canonical JSON of the descriptor, independent of authoring order and normalization", () => {
    const descriptor = {
        properties: {
          b: { kind: "counter" },
          a: { item: { properties: { z: { kind: "text" }, y: { kind: "boolean" } }, kind: "object" }, kind: "list" },
        },
        kind: "object",
    } as any;
    const golden =
      '{"kind":"object","properties":{"a":{"item":{"kind":"object","properties":{"y":{"kind":"boolean"},"z":{"kind":"text"}}},"kind":"list"},"b":{"kind":"counter"}}}';
    expect(schemaKey(descriptor)).toBe(golden);
    expect(schemaKey(fromDescriptor(descriptor).descriptor)).toBe(golden);
  });
  // Every released app.js compares this key with the shell's before it mounts, so the
  // spelling of every kind and option the builder emits is frozen.
  test("every kind and option keeps its released key", () => {
    const every = defineDocument({
      title: s.text(),
      done: s.boolean(),
      label: s.string({ maxLength: 40 }),
      ratio: s.number({ min: 0, max: 1.5 }),
      count: s.integer({ min: -3, max: 100 }),
      lane: s.enum(["todo", "doing", "done"]),
      hits: s.counter(),
      note: s.optional(s.string()),
      memo: s.optional(s.text()),
      place: s.optional(s.object({ name: s.string(), visits: s.integer({ min: 0 }) })),
      settings: s.object({ volume: s.integer({ min: 0, max: 10 }), muted: s.boolean() }),
      rows: s.list(s.object({ text: s.text(), tags: s.list(s.string()), notes: s.record(s.string()) })),
      colors: s.list(s.string()),
      cells: s.record(s.object({ input: s.text(), width: s.optional(s.integer()) })),
    });
    expect(schemaKey(every.descriptor)).toBe(
      '{"kind":"object","properties":{"cells":{"kind":"record","value":{"kind":"object","properties":{"input":{"kind":"text"},"width":{"inner":{"kind":"integer"},"kind":"optional"}}}},"colors":{"item":{"kind":"string"},"kind":"list"},"count":{"kind":"integer","max":100,"min":-3},"done":{"kind":"boolean"},"hits":{"kind":"counter"},"label":{"kind":"string","maxLength":40},"lane":{"kind":"enum","values":["todo","doing","done"]},"memo":{"inner":{"kind":"text"},"kind":"optional"},"note":{"inner":{"kind":"string"},"kind":"optional"},"place":{"inner":{"kind":"object","properties":{"name":{"kind":"string"},"visits":{"kind":"integer","min":0}}},"kind":"optional"},"ratio":{"kind":"number","max":1.5,"min":0},"rows":{"item":{"kind":"object","properties":{"notes":{"kind":"record","value":{"kind":"string"}},"tags":{"item":{"kind":"string"},"kind":"list"},"text":{"kind":"text"}}},"kind":"list"},"settings":{"kind":"object","properties":{"muted":{"kind":"boolean"},"volume":{"kind":"integer","max":10,"min":0}}},"title":{"kind":"text"}}}',
    );
  });
});

describe("bridge base64", () => {
  test("round-trips empty, odd and large payloads", () => {
    for (const size of [0, 1, 2, 3, 16_383, 16_384, 16_385, 100_001]) {
      const bytes = new Uint8Array(size).map((_, i) => (i * 31 + 7) & 255);
      const text = base64.encode(bytes);
      expect(text).toBe(Buffer.from(bytes).toString("base64"));
      expect(base64.decode(text)).toEqual(bytes);
    }
  });
  test("falls back without native Uint8Array helpers", () => {
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
});
