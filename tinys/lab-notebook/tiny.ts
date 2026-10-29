import { defineTiny } from "@hitslop/tinys";

export default defineTiny({
  slug: "lab-notebook",
  title: "Lab Notebook",
  description: "Type in measurements and get a clean figure for a lab report: scatter, line or bar, with error bars and an optional least-squares fit.",
  author: { name: "tinys", url: "https://tinys.click" },
  categories: ["education", "utilities"],
  window: { kind: "standard", width: 760, height: 880, resizable: true },
  theme: {
    paper: "#f3efe2",
    card: "#fbf9f1",
    ink: "#1d2433",
    muted: "#667085",
    rule: "#1d243326",
    grid: "#2f7d7a2e",
    series: "#1f4fb4",
    fit: "#c8372d",
  },
});
