import { defineDocument, s, type Value } from "@hitslop/document";

export const tones = ["coral", "sky", "mint", "butter", "lilac", "peach"] as const;
export const days = ["any", "mon", "tue", "wed", "thu", "fri", "sat", "sun"] as const;

const schema = defineDocument({
  title: s.text(),
  rules: s.text(),
  weeks: s.counter(),
  people: s.list(s.object({
    name: s.text(),
    tone: s.enum(tones),
    score: s.counter(),
  })),
  chores: s.list(s.object({
    name: s.text(),
    day: s.enum(days),
    points: s.integer({ min: 1, max: 5 }),
    rotates: s.boolean(),
    done: s.boolean(),
    assignee: s.optional(s.string()),
    doneBy: s.optional(s.string()),
  })),
});

export type Household = Value<typeof schema.descriptor>;
export type Person = Household["people"][number];
export type Chore = Household["chores"][number];
export default schema;
