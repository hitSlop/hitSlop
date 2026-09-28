import { defineDocument, s, type Value } from "@hitslop/document";

export const colors = ["mint", "coral", "butter", "blue", "lavender", "rose", "peach", "sage", "teal", "periwinkle", "amber", "stone"] as const;
export type HabitColor = (typeof colors)[number];
const schema = defineDocument({
  habits: s.list(s.object({
    name: s.text(),
    color: s.enum(colors),
    checkins: s.record(s.integer({ min: 1 })),
  })),
});
export type Habit = Value<typeof schema.fields.node>["habits"][number];
export default schema;
