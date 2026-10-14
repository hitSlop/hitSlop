import { defineDocument, s } from "hitslop";
import { signifierTypes } from "./signifiers";

export default defineDocument({
  date: s.text(),
  monthTitle: s.text(),
  dailyPage: s.text(),
  monthlyPage: s.text(),
  entries: s.list(s.object({
    type: s.enum(signifierTypes),
    star: s.boolean({ default: false }),
    text: s.text(),
  })),
  monthlyLog: s.list(s.object({
    day: s.integer({ min: 1, max: 31 }),
    weekday: s.text(),
    text: s.text(),
  })),
}, { initial: {
  date: "Wednesday, September 9, 2026",
  monthTitle: "September 2026",
  dailyPage: "42",
  monthlyPage: "40",
  entries: [
    { type: "task", star: true, text: "Ship the Eisenhower & Ivy Lee productivity dockets" },
    { type: "complete", text: "Morning review of server latency telemetry" },
    { type: "event", text: "14:00 Studio design sync with architectural team" },
    { type: "note", text: "A single physical balance beam clarifies weighted decisions" },
    { type: "migrated", text: "Update developer documentation on packaging invariants" },
    { type: "task", text: "Order heavier grain fountain pen ink refills" },
  ],
  monthlyLog: [
    { day: 1, weekday: "Tu", text: "Labor Day / Studio Planning" },
    { day: 2, weekday: "We", text: "Core templates sprint kickoff" },
    { day: 3, weekday: "Th", text: "Release candidate verification" },
    { day: 4, weekday: "Fr", text: "Team demo & retrospective" },
    { day: 12, weekday: "Sa", text: "Mountain trail run" },
    { day: 15, weekday: "Tu", text: "Quarterly budget audit" },
  ],
} });
