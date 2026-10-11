import { defineSlop } from "hitslop";

export default defineSlop({
  slug: "flashcards",
  title: "Flashcards",
  description: "A tactile index-card study machine with 3D flipping cards, deck editing, and Leitner spaced repetition.",
  author: { name: "hitSlop", url: "https://hitslop.com" },
  categories: ["education", "productivity"],
  window: { kind: "standard", width: 540, height: 440 },
  theme: {
    wood: "#284bd8",
    woodDeep: "#1b34a0",
    brassHi: "#a9baff",
    card: "#ffffff",
    cardLine: "#d8e2ff",
    cardWash: "#f4f7ff",
    margin: "#284bd8",
    ink: "#19295b",
    inkMuted: "#596887",
    ivory: "#ffffff",
    again: "#ffe3d8",
    good: "#b9f4db",
    danger: "#ffd5d8",
    overlay: "#0e1946a6",
    answer: "#e6f0ff",
  },
});
