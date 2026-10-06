import { expect, test } from "bun:test";
import { Check } from "typebox/value";
import { PageRequestSchema } from "../src/page";

test("page envelopes unify document and host methods with opaque core payloads", () => {
  for (const request of [
    { method: "open" },
    { method: "apply", batch: '{"intents":[]}' },
    { method: "text", request: "{}" },
    { method: "undo" },
    { method: "flush" },
    { method: "config" },
    { method: "attachments.read", attachmentID: "a".repeat(64) },
  ])
    expect(Check(PageRequestSchema, request)).toBe(true);
  for (const request of [
    { method: "apply", batch: {} },
    { method: "text" },
    { method: "config", extra: true },
  ])
    expect(Check(PageRequestSchema, request)).toBe(false);
});
