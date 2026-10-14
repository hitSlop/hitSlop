import { defineSlop } from "hitslop";

export default defineSlop({
  slug: "expense-log",
  title: "Expense Log",
  description: "A thermal paper receipt roll for rapidly recording purchases, tracking categories, and running totals.",
  author: { name: "hitSlop", url: "https://hitslop.com" },
  categories: ["finance", "utilities"],
  window: { kind: "standard", width: 380, height: 640 },
  theme: {
    paper: "#f7f1e3",
    paperShade: "#efe6d2",
    ticket: "#fffdf8",
    ink: "#1c1917",
    muted: "#5c564e",
    dim: "#8a847a",
    accent: "#c2410c",
    onAccent: "#fff7ed",
    danger: "#b91c1c",
    error: "#7b2630",
  },
});
