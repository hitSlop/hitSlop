import { defineDocument, s, type Value } from "@hitslop/document";

const schema = defineDocument({
  topic: s.text(),
  course: s.text(),
  lecturer: s.text(),
  summary: s.text(),
  date: s.string(),
  items: s.list(s.object({
    cue: s.text(),
    notes: s.text(),
  })),
  theme: s.enum(["light", "dark"]),
});

export type CornellNotes = Value<typeof schema.fields.node>;
export type CornellItem = CornellNotes["items"][number];
export default schema;
