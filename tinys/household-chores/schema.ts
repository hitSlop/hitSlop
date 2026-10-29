import { defineDocument, s } from "@hitslop/tinys";

export const tones = ["coral", "sky", "mint", "butter", "lilac", "peach"] as const;
export const days = ["any", "mon", "tue", "wed", "thu", "fri", "sat", "sun"] as const;

export default defineDocument({
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
    day: s.enum(days, { default: "any" }),
    points: s.integer({ min: 1, max: 5, default: 1 }),
    rotates: s.boolean({ default: true }),
    done: s.boolean({ default: false }),
    assignee: s.optional(s.string()),
    doneBy: s.optional(s.string()),
  })),
}, { initial: {
  title: "Our place",
  weeks: 0,
  rules: "Dishes before bed. Quiet after 11. Bins go out Sunday night.",
  people: [
    { name: "Mia", tone: "coral", score: 0 },
    { name: "Jo", tone: "sky", score: 0 },
    { name: "Sam", tone: "mint", score: 0 },
  ],
  chores: [
    { name: "Dishes", day: "any", points: 2 },
    { name: "Bins out", day: "sun", points: 1 },
    { name: "Bathroom", day: "sat", points: 4 },
    { name: "Vacuum", day: "wed", points: 3 },
    { name: "Groceries", day: "fri", points: 3 },
  ],
} });
