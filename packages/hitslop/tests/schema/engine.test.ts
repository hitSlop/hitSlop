import { expect, test } from "bun:test";
import { Check } from "typebox/value";
import { EngineRequestSchema, NativeRequestSchema, EngineReplySchema } from "../../src/schema/engine";

test("engine and renderer boundaries accept only their own requests", () => {
  const get = { method: "get", documentPath: "-Document with spaces.slop" };
  expect(Check(EngineRequestSchema, get)).toBe(true);
  expect(Check(NativeRequestSchema, get)).toBe(false);
  expect(Check(EngineRequestSchema, { ...get, protocol: 1 })).toBe(false);
  expect(Check(EngineRequestSchema, { method: "validateApp", app: { packageFormat: 999, manifest: { future: true } } })).toBe(true);
  const screenshot = { method: "screenshot", documentPath: "x.slop", output: "x.png", target: "icon", ifPresent: true };
  expect(Check(NativeRequestSchema, screenshot)).toBe(true);
  expect(Check(NativeRequestSchema, { ...screenshot, target: "unknown" })).toBe(false);
});

test("successes require method results, including explicit skipped artwork", () => {
  for (const reply of [
    { ok: true, method: "create" }, { ok: true, method: "templates", catalog: {} },
    { ok: true, method: "describe", state: {} }, { ok: true, method: "screenshot" },
  ]) expect(Check(EngineReplySchema, reply)).toBe(false);
  expect(Check(EngineReplySchema, { ok: true, method: "screenshot", output: null })).toBe(true);
  expect(Check(EngineReplySchema, { ok: false, code: "unknown_outcome", error: "Lost reply" })).toBe(true);
});
