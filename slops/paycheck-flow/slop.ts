import { defineSlop } from "hitslop";

export default defineSlop({
  slug: "paycheck-flow",
  title: "Paycheck Flow",
  description: "See where one month of income goes: money in on the left, flowing into rent, food, savings and the rest, with what's left over in gold.",
  author: { name: "hitSlop", url: "https://hitslop.com" },
  categories: ["finance"],
  window: { kind: "standard", width: 560, height: 780 },
  theme: {
    paper: "#f7f4ec",
    card: "#fffdf7",
    ink: "#1c2430",
    muted: "#6b7480",
    rule: "#1c243022",
    income: "#2e8f6d",
    hub: "#1c2430",
    gold: "#f0b429",
    warn: "#d9473b",
    sky: "#4c8dd6",
    leaf: "#4caf74",
    sun: "#f2b134",
    rose: "#e6628a",
    violet: "#8c6bd6",
    slate: "#6b7a8c",
  },
});
