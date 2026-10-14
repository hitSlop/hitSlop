import { defineDocument, s } from "hitslop";

export const colors = ["mint", "coral", "butter", "blue", "lavender", "rose", "peach", "sage", "teal", "periwinkle", "amber", "stone"] as const;

export default defineDocument({
  habits: s.list(s.object({
    name: s.text(),
    color: s.enum(colors),
    checkins: s.record(s.integer({ min: 1 }), { description: "Completed days, keyed by YYYY-MM-DD." }),
  })),
}, { initial: {
  habits: [
    { name: "Morning pages", color: "mint", checkins: {} },
    { name: "Walk outside", color: "coral", checkins: {} },
    { name: "Read 20 min", color: "butter", checkins: {} },
  ],
} });
