import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
  target: 2500,
  current: 1750,
  unit: "ml",
  logs: [
    { time: "08:30", amount: 250, label: "Morning glass" },
    { time: "11:00", amount: 500, label: "Desk bottle" },
    { time: "13:45", amount: 500, label: "Lunch bottle" },
    { time: "15:20", amount: 500, label: "Afternoon bottle" },
  ],
} satisfies Input<typeof schema.fields.node>;
