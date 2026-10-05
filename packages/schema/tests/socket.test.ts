import { test, expect } from "bun:test";
import { Check } from "typebox/value";
import { SocketRequestSchema, SocketDiscoverySchema, SocketReplySchema } from "../src/socket";

test("socket envelopes constrain routing while leaving operations to the document runtime", () => {
  const base = { documentPath: "/tmp/Test.slop" };
  expect(Check(SocketRequestSchema, { ...base, method: "hello" })).toBe(true);
  expect(Check(SocketRequestSchema, { ...base, method: "export", epoch: "session", format: "pdf", output: "/tmp/test.pdf" })).toBe(true);
  // Operations are JSON text that only the document core parses.
  expect(Check(SocketRequestSchema, { ...base, method: "batch", epoch: "session", ops: '{"type":"unknown-operation"}' })).toBe(true);
  for (const request of [
    { ...base, method: "unknown" },
    { ...base, method: "get", output: "/tmp/test.pdf" },
    { ...base, method: "batch", ops: "{}" },
    { ...base, method: "batch", epoch: "session", ops: null },
    { ...base, method: "batch", epoch: "session", ops: { type: "set" } },
  ]) expect(Check(SocketRequestSchema, request)).toBe(false);
  expect(Check(SocketDiscoverySchema, { socket: "/tmp/example.sock", documentPath: base.documentPath })).toBe(true);
  expect(Check(SocketDiscoverySchema, { socket: "/tmp/example.sock" })).toBe(false);
  expect(Check(SocketReplySchema, { ok: false, code: "save_failed", error: "save failed" })).toBe(true);
  expect(Check(SocketReplySchema, { ok: false, error: "save failed" })).toBe(false);
});

test("socket successes require their complete method result", () => {
  const theme = { defaults: { accent: "#123456" }, overrides: {}, effective: { accent: "#123456" } };
  const id = "a".repeat(64);
  const replies = [
    { ok: true, method: "hello", epoch: "owner", coreBuildId: "build" },
    { ok: true, method: "get", epoch: "owner", state: { schema: {}, state: { sequence: 0, version: "v", value: {}, issues: [], theme: {} } } },
    { ok: true, method: "batch", epoch: "owner", ids: [], sequence: 3 },
    { ok: true, method: "export", output: "/tmp/doc.pdf" },
    ...["theme.get", "theme.set", "theme.reset", "theme.import"].map((method) => ({ ok: true, method, epoch: "owner", state: theme })),
    { ok: true, method: "theme.export", epoch: "owner", state: { file: "{}" } },
    { ok: true, method: "attachments.list", epoch: "owner", state: [{ id, byteLength: 3 }] },
    { ok: true, method: "attachments.read", epoch: "owner", state: { bytes: "YWJj" } },
    { ok: true, method: "attachments.put", epoch: "owner", state: { id, byteLength: 3 } },
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
  expect(Check(SocketReplySchema, { ok: true, method: "batch", epoch: "owner", state: {}, sequence: 3 })).toBe(false);
});
