import { defineDocument, s, type Value } from "@hitslop/document";

const schema = defineDocument({
  date: s.string(),
  notes: s.text(),
  tasks: s.list(s.object({
    text: s.text(),
    done: s.boolean(),
  })),
});

export type IvyLee = Value<typeof schema.fields.node>;
export type IvyTask = IvyLee["tasks"][number];
export default schema;
