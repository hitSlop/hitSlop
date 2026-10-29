import { defineTiny } from "@hitslop/tinys";

export default defineTiny({
  slug: "pocket-sheet",
  title: "Pocket Sheet",
  description: "A little graph-paper spreadsheet for bill splits, tallies, and quick math.",
  author: { name: "tinys", url: "https://tinys.click" },
  categories: ["productivity", "personal"],
  window: { kind: "standard", width: 700, height: 640 },
  theme: {
    desk: "#e9e2d3",
    paper: "#fdfbf4",
    grid: "#c9dbe6",
    margin: "#e79a8e",
    ink: "#2f3a4a",
    pencil: "#6b7482",
    focus: "#2f6fd1",
    accent: "#e2553f",
    butter: "#fbe7a1",
    mint: "#c8ecd3",
    sky: "#cfe3fb",
    rose: "#f9d0d6",
    lilac: "#e2d6f6",
  },
});
