import { defineSlop } from "hitslop";

export default defineSlop({
  slug: "workout-planner",
  title: "Workout Planner",
  description: "A dark training console for logging individual sets, following your routine, and timing rest.",
  author: { name: "hitSlop", url: "https://hitslop.com" },
  categories: ["health", "personal"],
  window: { kind: "standard", width: 440, height: 620 },
  theme: {
    surface: "#101214",
    plate: "#191c1f",
    card: "#23272b",
    ink: "#f2f5ed",
    muted: "#a3aaa5",
    dim: "#7d8780",
    accent: "#c3f653",
    border: "#343b3e",
    onAccent: "#172006",
    danger: "#ff9393",
  },
});
