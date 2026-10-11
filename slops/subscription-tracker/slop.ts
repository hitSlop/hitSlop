import { defineSlop } from "hitslop";

export default defineSlop({
  slug: "subscription-tracker",
  title: "Subscription Tracker",
  description: "A calm local ledger for recurring costs, renewal dates, and monthly pace.",
  author: { name: "hitSlop", url: "https://hitslop.com" },
  categories: ["finance", "personal"],
  window: { kind: "standard", width: 640, height: 720 },
  theme: {
    surface: "#f4f3fa",
    paper: "#ffffff",
    paperSoft: "#efecfa",
    ink: "#242039",
    muted: "#696378",
    accent: "#6740c8",
    accentInk: "#292040",
    danger: "#b33655",
    mint: "#b6f3d8",
    onDark: "#ffffff",
    mutedOnDark: "#c6bed9",
  },
});
