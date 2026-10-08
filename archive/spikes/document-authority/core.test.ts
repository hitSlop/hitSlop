// New boundary: a non-CRDT reducer must reject stale/invalid batches atomically.
// Literal fixtures supply the independent oracle; production Loro tests do not exercise this core.
import { test, expect } from "bun:test";
import { apply, create, fixture, seed, type Op } from "./core";
import fixtures from "./fixtures.json";
for (const f of fixtures) test(f.name, () => expect(fixture(f)).toEqual(f.expected));
test("untrusted paths cannot mutate prototypes; inputs stay unchanged", () => {
  const before = create(seed(2)), serialized = JSON.stringify(before);
  expect(() => apply(before, [{type:"set",path:["__proto__","x"],value:true,expected:0}])).toThrow("invalid_path");
  expect(JSON.stringify(before)).toBe(serialized);
  expect(({} as any).x).toBeUndefined();
});
test("bounded batches reject before changing state", () => {
  const state = create(seed(1));
  expect(() => apply(state, Array.from({length:1025}, () => ({type:"set",path:["title"],value:"x",expected:0}) as Op))).toThrow("invalid_batch");
  expect(state.rev).toBe(0);
});
