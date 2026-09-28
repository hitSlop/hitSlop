import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
  storeName: "DAILY EXPENSE LOG",
  terminalId: "#0842",
  currency: "$",
  items: [
    { title: "Pour-over coffee", amount: 5.5, category: "coffee", date: "2026-09-16", time: "08:15" },
    { title: "Subway pass", amount: 2.9, category: "transit", date: "2026-09-16", time: "08:45" },
    { title: "Sourdough sandwich", amount: 14.25, category: "food", date: "2026-09-16", time: "12:30" },
    { title: "Notebook & fine pen", amount: 18, category: "gear", date: "2026-09-16", time: "15:10" },
    { title: "Cloud storage", amount: 9.99, category: "bills", date: "2026-09-16", time: "17:00" },
  ],
} satisfies Input<typeof schema.fields.node>;
