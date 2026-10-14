import { defineSlop } from "hitslop";

export default defineSlop({
  slug: "recipe",
  title: "Recipe",
  description: "A crisp recipe workspace with ingredients, method, meal photos, and focused step-by-step cooking timers.",
  author: { name: "hitSlop", url: "https://hitslop.com" },
  categories: ["personal", "popular"],
  window: { kind: "standard", width: 680, height: 680 },
  theme: {
    paper: "#ffffff",
    paperSoft: "#f1f4ff",
    ink: "#182338",
    muted: "#586279",
    accent: "#2448c8",
    accentSoft: "#e7edff",
    herb: "#c7d4ff",
    onAccent: "#ffffff",
    cook: "#142452",
    cookSoft: "#223665",
    cookMuted: "#c5cee5",
    tomato: "#c83c29",
  },
});
