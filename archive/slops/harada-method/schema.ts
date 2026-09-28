import { defineDocument, s, type Value } from "@hitslop/document";

const schema = defineDocument({
  goal: s.text(),
  deadline: s.string(),
  themes: s.list(s.object({
    title: s.text(),
    cells: s.list(s.string()),
  })),
  done: s.record(s.boolean()),
});

export type Sheet = Value<typeof schema.fields.node>;
export type Theme = Sheet["themes"][number];
export default schema;
