import { defineSlop } from "hitslop";

export default defineSlop({
  slug: "invoice",
  title: "Invoice",
  description: "A warm paper invoice for parties, line items, tax, and payment status.",
  author: { name: "hitSlop", url: "https://hitslop.com" },
  categories: ["business", "popular"],
  window: { kind: "standard", width: 640, height: 640 },
  theme: {
    surface: "#faf8f1",
    paper: "#faf8f1",
    ink: "#1b2421",
    muted: "#69736e",
    accent: "#2f7052",
    accentInk: "#1f4f39",
    warning: "#b77a2f",
    danger: "#a53a31",
  },
});
