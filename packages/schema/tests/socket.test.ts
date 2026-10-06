import { test, expect } from "bun:test";
import { Check } from "typebox/value";
import { SocketRequestSchema, SocketDiscoverySchema, SocketReplySchema } from "../src/socket";

test("socket envelopes constrain routing while leaving operations to the document runtime", () => {
  const base = { protocol: 1, documentPath: "/tmp/Test.slop" };
  expect(Check(SocketRequestSchema, { ...base, method: "get" })).toBe(true);
  expect(Check(SocketRequestSchema, { ...base, method: "export", format: "pdf", output: "/tmp/test.pdf" })).toBe(true);
  // Operations are JSON text that only the document core parses.
  expect(Check(SocketRequestSchema, { ...base, method: "batch", ops: '{"type":"unknown-operation"}' })).toBe(true);
  for (const request of [
    { ...base, method: "unknown" },
    { ...base, method: "get", output: "/tmp/test.pdf" },
    { documentPath: base.documentPath, method: "batch", ops: "{}" },
    { ...base, method: "batch", ops: null },
    { ...base, method: "batch", ops: { type: "set" } },
  ]) expect(Check(SocketRequestSchema, request)).toBe(false);
  expect(Check(SocketDiscoverySchema, { socket: "/tmp/example.sock", documentPath: base.documentPath })).toBe(true);
  expect(Check(SocketDiscoverySchema, { socket: "/tmp/example.sock" })).toBe(false);
  expect(Check(SocketReplySchema, { ok: false, code: "save_failed", error: "save failed" })).toBe(true);
  expect(Check(SocketReplySchema, { ok: false, error: "save failed" })).toBe(false);
});

test("socket successes require their complete method result", () => {
  const id = "a".repeat(64);
  const replies = [
    { ok: true, method: "get", state: { schema: {}, defaults: { accent: "#335577" }, version: "v", value: {}, theme: { accent: "#123456" } } },
    { ok: true, method: "batch", ids: [] },
    { ok: true, method: "export", output: "/tmp/doc.pdf" },
    { ok: true, method: "theme.export", state: { file: "{}" } },
    { ok: true, method: "attachments.list", state: [{ id, byteLength: 3 }] },
    { ok: true, method: "attachments.read", state: { bytes: "YWJj" } },
  ];
  for (const reply of replies) {
    expect(Check(SocketReplySchema, reply)).toBe(true);
    for (const key of Object.keys(reply)) {
      const incomplete = { ...reply } as Record<string, unknown>;
      delete incomplete[key];
      expect(Check(SocketReplySchema, incomplete)).toBe(false);
    }
    expect(Check(SocketReplySchema, { ...reply, error: "contradictory success" })).toBe(false);
  }
  expect(Check(SocketReplySchema, { ok: false, code: "unknown_outcome", error: "Disconnected" })).toBe(true);
  // The page's publication sequence and a batch's version are not part of an agent's reply.
  expect(Check(SocketReplySchema, { ok: true, method: "batch", ids: [], sequence: 3 })).toBe(false);
  expect(Check(SocketReplySchema, { ok: true, method: "batch", ids: [], version: "00" })).toBe(false);
  const get = replies[0] as { state: Record<string, unknown> };
  expect(Check(SocketReplySchema, { ...get, state: { ...get.state, sequence: 0 } })).toBe(false);
});
