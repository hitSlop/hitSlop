import { defineDocument, s, type Value } from "@hitslop/document";

const schema = defineDocument({
  prompt: s.text(),
  entries: s.list(s.object({
    day: s.string(),
    mood: s.integer({ min: 1, max: 5 }),
    energy: s.integer({ min: 1, max: 5 }),
    note: s.text(),
  })),
});

export type MoodLog = Value<typeof schema.fields.node>;
export default schema;
