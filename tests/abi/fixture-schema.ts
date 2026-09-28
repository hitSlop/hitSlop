// Shared by the plain and Svelte ABI consumers and the baseline data fixtures.
// Covers every value kind: text, rich text, counter, scalars with bounds and enums,
// scalar lists, object rows, records, trees and optional composites.
import { defineDocument, s, type Input } from "@hitslop/document";

export const conformance = defineDocument({
  title: s.text(),
  notes: s.richtext({ bold: "after" }),
  count: s.counter(),
  done: s.boolean(),
  level: s.integer({ min: 0, max: 10 }),
  mode: s.enum(["a", "b", "c"]),
  tags: s.list(s.string()),
  rows: s.list(s.object({ name: s.text(), done: s.boolean() })),
  cells: s.record(s.string()),
  outline: s.tree(s.object({ label: s.text() })),
  cover: s.optional(s.object({ caption: s.string() })),
});
export default conformance;

export const initial: Input<typeof conformance.fields.node> = {
  title: "Conformance",
  notes: "Hello 🌍",
  count: 1,
  done: false,
  level: 3,
  mode: "a",
  tags: ["one"],
  rows: [
    { name: "Alpha", done: false },
    { name: "Beta", done: true },
  ],
  cells: { A1: "1" },
  outline: [{ label: "Root", children: [{ label: "Leaf" }] }],
};
