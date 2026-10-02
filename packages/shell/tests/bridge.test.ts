import { expect, test } from "bun:test";
import { call } from "../src/bridge";
import { isDocumentError, isRejected } from "@hitslop/document";

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
              return entry.reply;
            },
          },
        },
      };
      const error = await call({ method: "apply", batch: '{"intents":[]}' }).catch(
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
