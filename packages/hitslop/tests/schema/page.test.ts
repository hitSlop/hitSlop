import { expect, test } from "bun:test";
import { Check } from "typebox/value";
import { PageRequestSchema } from "../../src/schema/page";

test("page envelopes unify document and host methods with opaque core payloads", () => {
  for (const request of [
    { method: "open" },
    { method: "apply", batch: '{"intents":[]}' },
    { method: "undo" },
    { method: "flush" },
    { method: "config" },
    { method: "attachments.read", attachmentID: "a".repeat(64) },
  ])
    expect(Check(PageRequestSchema, request)).toBe(true);
  for (const request of [
    { method: "apply", batch: {} },
    // Text edits are batches: a set from the batch's base.
    { method: "text", request: "{}" },
    { method: "config", extra: true },
  ])
    expect(Check(PageRequestSchema, request)).toBe(false);
});
