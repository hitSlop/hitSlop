import { defineDocument, s, type Value } from "@hitslop/document";

const schema = defineDocument({
  currentKey: s.string(),
  entries: s.record(s.object({
    date: s.string(),
    text: s.text(),
    completedAt: s.string(),
  })),
});

export type MorningPages = Value<typeof schema.fields.node>;
export type DayEntry = MorningPages["entries"][string];
export default schema;
