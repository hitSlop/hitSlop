import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
  title: "Our place",
  weeks: 0,
  rules: "Dishes before bed. Quiet after 11. Bins go out Sunday night.",
  people: [
    { name: "Mia", tone: "coral", score: 0 },
    { name: "Jo", tone: "sky", score: 0 },
    { name: "Sam", tone: "mint", score: 0 },
  ],
  chores: [
    { name: "Dishes", day: "any", points: 2, rotates: true, done: false },
    { name: "Bins out", day: "sun", points: 1, rotates: true, done: false },
    { name: "Bathroom", day: "sat", points: 4, rotates: true, done: false },
    { name: "Vacuum", day: "wed", points: 3, rotates: true, done: false },
    { name: "Groceries", day: "fri", points: 3, rotates: true, done: false },
  ],
} satisfies Input<typeof schema.descriptor>;
