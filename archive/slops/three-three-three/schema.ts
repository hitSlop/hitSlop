import { defineDocument, s, type Value } from "@hitslop/document";

const task = s.object({
  text: s.text(),
  done: s.boolean(),
});

const schema = defineDocument({
  date: s.string(),
  deepWork: s.object({
    project: s.text(),
    minutes: s.integer({ min: 0, max: 180 }),
    notes: s.text(),
  }),
  shortTasks: s.list(task),
  maintenance: s.list(task),
});

export type ThreeThreeThree = Value<typeof schema.fields.node>;
export type PlanTask = ThreeThreeThree["shortTasks"][number];
export default schema;
