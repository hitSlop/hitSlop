import { defineSlop } from "hitslop";

export default defineSlop({
  slug: "tiny-planet",
  title: "Tiny Planet",
  description: "A little low-poly world to spin and decorate: plant trees, build houses, drop crystals, and watch the seasons change.",
  author: { name: "hitSlop", url: "https://hitslop.com" },
  categories: ["creative", "games"],
  window: { kind: "standard", width: 480, height: 560 },
  theme: {
    space: "#0b1026",
    nebula: "#262a66",
    ocean: "#2f7fb5",
    sand: "#e8d29a",
    grass: "#7fc46a",
    forest: "#3f8f4e",
    rock: "#8a8f9c",
    snow: "#f4f8fb",
    autumn: "#e08a3a",
    blossom: "#f2a7c3",
    wall: "#f6e9d2",
    roof: "#d9553c",
    trunk: "#7a5236",
    crystal: "#7de3ff",
    ink: "#f3f5ff",
    accent: "#ffd166",
    glass: "#ffffff1f",
  },
});
