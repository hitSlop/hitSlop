import { defineDocument, s, type Value } from "@hitslop/document";

export const colors = ["sky", "coral", "mint", "lilac"] as const;
export type TaskColor = (typeof colors)[number];

const schema = defineDocument({
  week: s.text(),
  focus: s.text(),
  days: s.list(s.object({
    name: s.string(),
    date: s.string(),
    tasks: s.list(s.object({
      time: s.string(),
      title: s.text(),
      done: s.boolean(),
      durationMinutes: s.optional(s.integer({ min: 15, max: 1440 })),
      color: s.enum(colors),
    })),
  })),
});

export type Planner = Value<typeof schema.fields.node>;
export type Day = Planner["days"][number];
export type Task = Day["tasks"][number];
export default schema;
