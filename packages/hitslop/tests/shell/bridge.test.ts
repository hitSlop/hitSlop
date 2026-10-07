import { describe, expect, test } from "bun:test";
import { base64, call } from "../../src/shell/bridge";
import { isDocumentError, isRejected } from "hitslop";

test("native refusals and transport failures retain distinct outcomes", async () => {
  const root = globalThis as any,
    before = root.webkit;
  const cases = [
    {
      reply: { ok: false, code: "rejected", error: "bad", reason: "out_of_range", opIndex: 1 },
      code: "rejected",
    },
    { reply: { ok: false, code: "closing", error: "closing" }, code: "closing" },
    { reply: { ok: false, code: "save_failed", error: "disk" }, code: "save_failed" },
    { reply: { ok: true, method: "undo", sequence: 1 }, code: "unknown_outcome" },
    { reply: { ok: "true" }, code: "unknown_outcome" },
    { reply: { ok: false, code: "rejected" }, code: "unknown_outcome" },
    { reply: [], code: "unknown_outcome" },
    { reply: null, code: "unknown_outcome" },
    { reply: { ok: false, code: "bogus", error: "bad" }, code: "unknown_outcome" },
    { reply: undefined, code: "unknown_outcome" },
  ];
  try {
    for (const entry of cases) {
      root.webkit = {
        messageHandlers: {
          hitslop: {
            postMessage: async () => {
              if (entry.reply === undefined) throw new Error("lost connection");
              return JSON.stringify(entry.reply);
            },
          },
        },
      };
      const error = await call({ method: "apply", batch: { intents: [] } }).catch(
        (error) => error,
      );
      expect(isDocumentError(error)).toBe(true);
      expect(error.code).toBe(entry.code);
      expect(isRejected(error)).toBe(entry.code === "rejected");
      if (isRejected(error)) {
        expect(error.reason).toBe("out_of_range");
        expect(error.opIndex).toBe(1);
      }
    }
  } finally {
    root.webkit = before;
  }
});

// Attachment bytes cross the bridge as base64; WebKit before Safari 18.2 (macOS 15.0)
// has no native Uint8Array codec.
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

test("native bridge carries JSON text and correlates a successful reply", async () => {
  const root = globalThis as any, before = root.webkit;
  try {
    root.webkit = { messageHandlers: { hitslop: { postMessage: async (json: string) => {
      expect(typeof json).toBe("string");
      expect(JSON.parse(json)).toEqual({ method: "apply", batch: { intents: [] } });
      return JSON.stringify({ ok: true, method: "apply", sequence: 2, ids: ["row"] });
    } } } };
    expect(await call({ method: "apply", batch: { intents: [] } })).toEqual({ sequence: 2, ids: ["row"] });
    for (const malformed of ["{", { ok: true, method: "apply" }]) {
      root.webkit.messageHandlers.hitslop.postMessage = async () => malformed;
      await expect(call({ method: "apply", batch: { intents: [] } })).rejects.toMatchObject({ code: "unknown_outcome" });
    }
  } finally { root.webkit = before; }
});
