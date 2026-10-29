import { defineTiny } from "@hitslop/tinys";

export default defineTiny({
  slug: "small-expenses",
  title: "Small Expenses",
  description: "Track a short expense list and settle selected items.",
  author: { name: "tinys", url: "https://tinys.click" },
  categories: ["productivity"],
  window: { kind: "standard", width: 640, height: 760 },
  theme: {
    surface: "#faf7f0",
    ink: "#312c29",
    muted: "#756a60",
    rule: "#ddd4c8",
    accent: "#8b4e31",
    panel: "#eee5d8",
  },
});
