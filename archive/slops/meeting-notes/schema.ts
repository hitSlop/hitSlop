import { defineDocument, s, type Value } from "@hitslop/document";

const schema = defineDocument({
  title: s.text(),
  date: s.string(),
  time: s.string(),
  attendees: s.list(s.string()),
  agenda: s.list(s.object({ text: s.text(), done: s.boolean() })),
  decisions: s.list(s.object({ text: s.text() })),
  notes: s.text(),
  actions: s.list(s.object({ text: s.text(), owner: s.text(), done: s.boolean() })),
});

export type Meeting = Value<typeof schema.fields.node>;
export default schema;
