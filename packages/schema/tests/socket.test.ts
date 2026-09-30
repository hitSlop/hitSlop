import { test, expect } from "bun:test";
import { Check } from "typebox/value";
import { SocketRequestSchema, SocketDiscoverySchema, SocketReplySchema } from "../src/socket";

test("socket envelopes constrain routing while leaving operations to the document runtime", () => {
  const base = { id: "request", documentPath: "/tmp/Test.slop" };
  expect(Check(SocketRequestSchema, { ...base, method: "hello" })).toBe(true);
  expect(Check(SocketRequestSchema, { ...base, method: "export", epoch: "session", format: "pdf", output: "/tmp/test.pdf" })).toBe(true);
  expect(Check(SocketRequestSchema, { ...base, method: "apply", epoch: "session", op: { type: "unknown-operation" } })).toBe(true);
  for (const request of [
    { ...base, method: "unknown" },
    { ...base, method: "get", output: "/tmp/test.pdf" },
    { ...base, method: "apply", op: {} },
    { ...base, method: "batch", epoch: "session", ops: null },
    { ...base, method: "apply", epoch: "session", op: [] },
  ]) expect(Check(SocketRequestSchema, request)).toBe(false);
  expect(Check(SocketDiscoverySchema, { socket: "/tmp/example.sock", documentPath: base.documentPath })).toBe(true);
  expect(Check(SocketDiscoverySchema, { socket: "/tmp/example.sock" })).toBe(false);
  expect(Check(SocketReplySchema, { ok: false, error: "save failed" })).toBe(true);
});
