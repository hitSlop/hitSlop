import { defineDocument, s } from "hitslop";

export default defineDocument({
  target: s.number({ description: "Daily goal, in the document's unit." }),
  unit: s.string(),
  logs: s.list(s.object({
    time: s.string({ description: "Local time of the drink, HH:MM." }),
    amount: s.number(),
    label: s.string(),
  })),
}, { initial: {
  target: 2500,
  unit: "ml",
  logs: [
    { time: "08:30", amount: 250, label: "Morning glass" },
    { time: "11:00", amount: 500, label: "Desk bottle" },
    { time: "13:45", amount: 500, label: "Lunch bottle" },
    { time: "15:20", amount: 500, label: "Afternoon bottle" },
  ],
} });
