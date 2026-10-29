import { defineTiny } from "@hitslop/tinys";

export default defineTiny({
  slug: "cornell-notes",
  title: "Cornell Notes",
  description: "A classic Cornell lecture notebook page with cue column, notes area, summary box, and an active-recall study mask.",
  author: { name: "tinys", url: "https://tinys.click" },
  categories: ["education", "productivity"],
  window: { kind: "standard", width: 700, height: 900 },
  theme: {
    surface: "#ffffff",
    paper: "#f8fafc",
    ink: "#0f172a",
    muted: "#64748b",
    accent: "#b31b1b",
    onAccent: "#ffffff",
    navy: "#1e3a8a",
    line: "#cbd5e1",
    summary: "#b45309",
  },
});
