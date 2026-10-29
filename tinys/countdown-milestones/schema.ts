import { defineDocument, s } from "@hitslop/tinys";

export default defineDocument({
  title: s.text(),
  createdAt: s.string(),
  targetDate: s.string({ description: "Event date as YYYY-MM-DD, local time." }),
  targetTime: s.string({ description: "Event time as HH:MM, local time." }),
  milestones: s.list(s.object({
    title: s.text(),
    done: s.boolean({ default: false }),
  })),
}, { initial: {
  title: "The big trip",
  createdAt: "2026-09-16",
  targetDate: "2026-12-12",
  targetTime: "09:00",
  milestones: [
    { title: "Book the stay" },
    { title: "Plan a few adventures" },
    { title: "Pack the essentials" },
  ],
} });
