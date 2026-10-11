import { defineSlop } from "hitslop";

export default defineSlop({
  slug: "small-expenses",
  title: "Small Expenses",
  description: "Track a short expense list and settle selected items.",
  author: { name: "hitSlop", url: "https://hitslop.com" },
  categories: ["productivity"],
  window: { kind: "standard", width: 640, height: 760 },
  theme: {
    surface: "#faf7f0",
    ink: "#312c29",
    muted: "#756a60",
    rule: "#ddd4c8",
    accent: "#8b4e31",
    panel: "#eee5d8",
  },
});
