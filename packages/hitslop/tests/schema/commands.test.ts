import { expect, test } from "bun:test";
import { CommandMetadata } from "../../src/schema/commands";
import { validate } from "../../src/schema/validation";

test("stored command metadata has only callable command names and bounded entries", () => {
  const spec = { description: "Do something", args: { type: "object", properties: {}, additionalProperties: false } };
  expect(() => validate(CommandMetadata, { doSomething: spec })).not.toThrow();
  for (const metadata of [
    { "not-a-command": spec },
    { ["a".repeat(81)]: spec },
    Object.fromEntries(Array.from({ length: 65 }, (_, index) => [`command${index}`, spec])),
  ]) expect(() => validate(CommandMetadata, metadata)).toThrow();
});
