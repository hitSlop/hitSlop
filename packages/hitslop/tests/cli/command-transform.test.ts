import { test, expect } from "bun:test";
import { stripCommandBodies } from "../../src/cli/command-transform";

const strip = (source: string, file = "actions.ts") => stripCommandBodies(source, file)?.code;

test("document command declarations lose only their body", () => {
  const code = strip(`import doc from './model';
export const rename = doc.command({ description: "Rename", args: {}, run({ tx }) {
  // run({ tx }) { } in a comment, "run" in a string, and nested { braces }
  const sentinel = "BODY_ONLY"; return { sentinel };
} });`)!;
  expect(code).not.toContain("BODY_ONLY");
  expect(code).toContain('doc.command({ description: "Rename", args: {} })');
});

test("other .command calls and aliased declarations are left alone", () => {
  for (const source of [
    `program.command("serve").action(() => run());`,
    `cli.command({ name: "x", run() { return "KEEP"; } });`,
    `cli.command({ description: "x", run() { return "KEEP"; } });`,
    `const declare = doc.command; export const c = declare({ description: "x", args: {}, run() { return "KEEP"; } });`,
  ]) expect(strip(source)).toBeUndefined();
});

test("JSX modules are transformed like TypeScript", () => {
  const code = strip(`export const c = doc.command({ description: "x", args: {}, run: () => "BODY" }); export const v = <b/>;`, "view.tsx")!;
  expect(code).not.toContain("BODY");
});

test("a command declaration in another shape is an authoring error, not a shipped body", () => {
  for (const source of [
    `doc.command({ ...shared, args: {}, run() { return "BODY"; } });`,
    `doc.command({ description: "x", args: {}, examples: [], run() { return "BODY"; } });`,
    `doc.command({ args: {}, run() { return "BODY"; } });`,
  ]) expect(() => strip(source)).toThrow("exactly those fields");
});

test("an asynchronous body is an authoring error", () => {
  expect(() => strip(`doc.command({ description: "x", args: {}, async run() {} });`)).toThrow("synchronous");
  expect(() => strip(`doc.command({ description: "x", args: {}, run: async () => 1 });`)).toThrow("synchronous");
});
