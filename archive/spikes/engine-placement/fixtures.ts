import { defineDocument, s } from "../../packages/document/src/schema";
export const checklist = defineDocument({
  title: s.text(),
  rows: s.list(s.object({ text: s.text(), done: s.boolean() })),
});
export const richtext = defineDocument({
  title: s.text(),
  body: s.richtext({ bold: "after" }),
  rows: s.list(s.object({ text: s.text(), done: s.boolean() })),
});
export const mixed = defineDocument({
  title: s.text(),
  body: s.richtext({ bold: "after" }),
  rows: s.list(s.object({ text: s.text(), done: s.boolean() })),
  taps: s.counter(),
  labels: s.record(s.string()),
  note: s.optional(s.text()),
  tree: s.tree(s.object({ name: s.text() })),
});
export function initial(rows: number, kind = "checklist") {
  return {
    title: "Engine placement",
    rows: Array.from({ length: rows }, (_, i) => ({ text: `Row ${i}`, done: false })),
    ...(kind !== "checklist" ? { body: "Hello collaborative world" } : {}),
    ...(kind === "mixed" ? { taps: 0, labels: { color: "blue" }, tree: [] } : {}),
  };
}
