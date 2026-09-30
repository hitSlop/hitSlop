// Guards the canonical schema key (the host refuses a document whose stored key differs)
// and the base64 codec used for attachment bytes on the host bridge.
import { describe, test, expect } from "bun:test";
import { fromDescriptor, schemaKey } from "../src/schema";
import { base64 } from "../src/bridge";

describe("schema identity", () => {
  // Stored documents open only under an identical key, so these rules are frozen:
  // keys sorted recursively, array order kept, no whitespace, JavaScript JSON numbers.
  test("keys are canonical JSON of the descriptor, independent of authoring order and normalization", () => {
    const descriptor = {
      root: {
        properties: {
          b: { kind: "counter" },
          a: { item: { properties: { z: { kind: "text" }, y: { kind: "boolean" } }, kind: "object" }, kind: "list" },
        },
        kind: "object",
      },
      format: 1,
    } as any;
    const golden =
      '{"format":1,"root":{"kind":"object","properties":{"a":{"item":{"kind":"object","properties":{"y":{"kind":"boolean"},"z":{"kind":"text"}}},"kind":"list"},"b":{"kind":"counter"}}}}';
    expect(schemaKey(descriptor)).toBe(golden);
    expect(schemaKey(fromDescriptor(descriptor).descriptor)).toBe(golden);
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
