import { defineTiny } from "@hitslop/tinys";

export default defineTiny({
  slug: "mash",
  title: "MASH",
  description: "The sleepover fortune game on a notebook page: fill in your futures, draw the spiral, and see where you end up.",
  author: { name: "tinys", url: "https://tinys.click" },
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
