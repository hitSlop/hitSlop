import { defineSlop } from "hitslop";

export default defineSlop({
  slug: "mash",
  title: "MASH",
  description: "The sleepover fortune game on a notebook page: fill in your futures, draw the spiral, and see where you end up.",
  author: { name: "hitSlop", url: "https://hitslop.com" },
  categories: ["games", "personal"],
  window: { kind: "standard", width: 520, height: 760 },
  theme: {
    desk: "#2b2f4a",
    paper: "#fdfdf7",
    rule: "#a9cdea",
    margin: "#ee8c8c",
    ink: "#27408b",
    pink: "#e0368c",
    highlight: "#fff17a",
    pencil: "#55565c",
    coil: "#9aa0ad",
  },
});
