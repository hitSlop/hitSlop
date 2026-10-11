import { defineSlop } from "hitslop";

export default defineSlop({
  slug: "equation-notebook",
  title: "Equation Notebook",
  description: "Work through maths one typeset line at a time on green engineering paper, with a live graph beside any line and sliders for its letters.",
  author: { name: "hitSlop", url: "https://hitslop.com" },
  categories: ["education", "productivity"],
  window: { kind: "standard", resizable: true, fullscreenable: true, width: 640, height: 780 },
  theme: {
    paper: "#eef3e1",
    grid: "#cddfc0",
    header: "#dfe9cc",
    margin: "#cf8f7c",
    ink: "#1e2b23",
    muted: "#55685b",
    pencil: "#3b4a40",
    curve: "#c2412d",
    accent: "#255f86",
    highlight: "#f5e48c",
    plotPaper: "#fbfcf6",
  },
});
