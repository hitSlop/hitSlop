import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
  date: "Wednesday, September 9, 2026",
  monthTitle: "September 2026",
  dailyPage: "42",
  monthlyPage: "40",
  entries: [
    { type: "task", star: true, text: "Ship the Eisenhower & Ivy Lee productivity dockets" },
    { type: "complete", star: false, text: "Morning review of server latency telemetry" },
    { type: "event", star: false, text: "14:00 Studio design sync with architectural team" },
    { type: "note", star: false, text: "A single physical balance beam clarifies weighted decisions" },
    { type: "migrated", star: false, text: "Update developer documentation on packaging invariants" },
    { type: "task", star: false, text: "Order heavier grain fountain pen ink refills" },
  ],
  monthlyLog: [
    { day: 1, weekday: "Tu", text: "Labor Day / Studio Planning" },
    { day: 2, weekday: "We", text: "Core templates sprint kickoff" },
    { day: 3, weekday: "Th", text: "Release candidate verification" },
    { day: 4, weekday: "Fr", text: "Team demo & retrospective" },
    { day: 12, weekday: "Sa", text: "Mountain trail run" },
    { day: 15, weekday: "Tu", text: "Quarterly budget audit" },
  ],
} satisfies Input<typeof schema.fields.node>;
