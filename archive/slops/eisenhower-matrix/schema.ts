import { defineDocument, s, type Value } from "@hitslop/document";

export const zones = ["inbox", "q1", "q2", "q3", "q4"] as const;

const schema = defineDocument({
  title: s.text(),
  date: s.text(),
  tasks: s.list(s.object({
    text: s.text(),
    done: s.boolean(),
    zone: s.enum(zones),
  })),
});

export type Matrix = Value<typeof schema.fields.node>;
export type Task = Matrix["tasks"][number];
export type Zone = Task["zone"];
export default schema;
