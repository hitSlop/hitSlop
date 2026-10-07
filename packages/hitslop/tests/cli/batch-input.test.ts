import { expect, test } from "bun:test";
import { batchIntents } from "../../src/cli/documents";

test("CLI batch numbers reach Rust without JavaScript rounding", () => {
  for (const token of ["9007199254740991", "9007199254740993", "-9007199254740993", "1e300", "1e999", "-0.0", "1.0000000000000001"]) {
    const input = `[{"type":"set","path":["amount"],"value":${token}}]`;
    expect(JSON.stringify({ intents: batchIntents(input) })).toBe(`{"intents":${input}}`);
  }
  expect(() => batchIntents("{}")).toThrow("array");
  expect(() => batchIntents("[")).toThrow();
});
