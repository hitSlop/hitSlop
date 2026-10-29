import { defineTiny } from "@hitslop/tinys";

export default defineTiny({
  slug: "quick-checklist",
  title: "Quick Checklist",
  description: "A blush pocket utility for capturing, finishing, and filing short task lists.",
  author: { name: "tinys", url: "https://tinys.click" },
  categories: ["productivity", "popular"],
  window: { kind: "standard", fullscreenable: true, width: 480, height: 620 },
  theme: {
    surface: "#e98996",
    paper: "#fff9f3",
    ink: "#432830",
    muted: "#82636a",
    accent: "#a43d59",
    onAccent: "#432830",
    rule: "#ead9cb",
    control: "#f1dfc9",
    highlight: "#f8d65a",
    success: "#94d7c0",
    successBorder: "#6c354b",
    filed: "#fff5e9",
    controlBorder: "#d8c0af",
  },
});
