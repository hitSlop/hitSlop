import { defineSlop } from "hitslop";

export default defineSlop({
  slug: "mood-log",
  title: "Mood Log",
  description: "A gentle color journal for recording daily mood, energy, and a short reflection without pressure.",
  author: { name: "hitSlop", url: "https://hitslop.com" },
  categories: ["health", "personal"],
  window: { kind: "standard", width: 480, height: 620 },
  theme: {
    surface: "#fcfbfa",
    panel: "#f5f2eb",
    paper: "#ffffff",
    ink: "#26221f",
    muted: "#6e6761",
    dim: "#a39c94",
    border: "#e6dfd1",
    orb1: "#a78bfa",
    orb2: "#38bdf8",
    orb3: "#4ade80",
    orb4: "#facc15",
    orb5: "#fb7185",
  },
});
