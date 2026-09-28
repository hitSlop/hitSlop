import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
  month: "September 2026",
  income: 5400,
  savings: 1200,
  categories: [
    { name: "Housing", allocated: 1600, spent: 1600 },
    { name: "Food & Dining", allocated: 800, spent: 542 },
    { name: "Transport", allocated: 300, spent: 212 },
    { name: "Utilities", allocated: 350, spent: 315 },
    { name: "Health", allocated: 200, spent: 120 },
    { name: "Other & Leisure", allocated: 400, spent: 270 },
  ],
} satisfies Input<typeof schema.fields.node>;
