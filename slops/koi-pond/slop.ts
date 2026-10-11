import { defineSlop } from "hitslop";

export default defineSlop({
  slug: "koi-pond",
  title: "Koi Pond",
  description: "A little watercolor pond where your koi drift, nibble, and keep you company.",
  author: { name: "hitSlop", url: "https://hitslop.com" },
  categories: ["creative", "games"],
  window: { kind: "standard", width: 420, height: 420, shape: "50%", background: "transparent" },
  theme: {
    water: "#5f9fa3",
    deep: "#2c5f6b",
    shallow: "#a9d3c6",
    stone: "#9a978c",
    moss: "#6e8f4e",
    lily: "#5f9a4a",
    lotus: "#f2a3b3",
    paper: "#fbf6ea",
    ink: "#23363a",
    focus: "#f6c64e",
    cream: "#fbf3e6",
    red: "#e2463a",
    gold: "#f2b33d",
    charcoal: "#2b2a2e",
    slate: "#7d97b8",
    orange: "#ef8a4a",
  },
});
