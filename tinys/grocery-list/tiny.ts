import { defineTiny } from "@hitslop/tinys";

export default defineTiny({
  slug: "grocery-list",
  title: "Grocery List",
  description: "A playful fridge-magnet shopping pad for organizing and checking grocery essentials by aisle.",
  author: { name: "tinys", url: "https://tinys.click" },
  categories: ["personal", "popular"],
  window: { kind: "standard", width: 440, height: 620 },
  theme: {
    surface: "#e9e4d8",
    paper: "#fffefb",
    ink: "#232220",
    muted: "#6e6b66",
    dim: "#a39f97",
    accent: "#d9534f",
    onAccent: "#ffffff",
    rule: "#6473821f",
    control: "#f4f2eb",
    sticky: "#fde8ec",
    stickyBorder: "#fbc6d0",
    stickyInk: "#881337",
    banana: "#facc15",
    bananaTip: "#854d0e",
    star: "#38bdf8",
    border: "#d2ccbf",
    pin: "#ef4444",
  },
});
