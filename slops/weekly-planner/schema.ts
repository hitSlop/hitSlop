import { defineDocument, s } from "hitslop";

export const colors = ["sky", "coral", "mint", "lilac"] as const;

const task = (time: string, title: string, color: (typeof colors)[number], durationMinutes: number, done = false) =>
  ({ time, title, done, durationMinutes, color });

export default defineDocument({
  week: s.text(),
  focus: s.text(),
  days: s.list(s.object({
    name: s.string(),
    date: s.string(),
    tasks: s.list(s.object({
      time: s.string({ description: "Start as HH:MM, or empty for an anytime block." }),
      title: s.text(),
      done: s.boolean({ default: false }),
      durationMinutes: s.optional(s.integer({ min: 15, max: 1440 })),
      color: s.enum(colors),
    })),
  })),
}, { initial: {
  week: "Week of Oct 12 — 18",
  focus: "Ship the v2 release & celebrate.",
  days: [
    { name: "MON", date: "12", tasks: [task("09:00", "Team sync", "sky", 60, true), task("14:00", "Design review", "lilac", 90, true)] },
    { name: "TUE", date: "13", tasks: [task("10:30", "Customer interview", "coral", 60, true), task("15:00", "API integration", "mint", 60)] },
    { name: "WED", date: "14", tasks: [task("09:00", "Refactor models", "sky", 90), task("13:30", "Run test suite", "lilac", 60)] },
    { name: "THU", date: "15", tasks: [task("11:00", "Staging deploy", "coral", 60), task("16:00", "QA pass", "mint", 90)] },
    { name: "FRI", date: "16", tasks: [task("10:00", "Production launch", "sky", 60), task("17:00", "Team demo & toast", "lilac", 60)] },
    { name: "SAT", date: "17", tasks: [task("09:30", "Farmers market", "coral", 90)] },
    { name: "SUN", date: "18", tasks: [task("", "Plan next week", "mint", 60)] },
  ],
} });
