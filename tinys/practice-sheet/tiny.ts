import { defineTiny } from "@hitslop/tinys";

export default defineTiny({
  slug: "practice-sheet",
  title: "Practice Sheet",
  description: "Learn a piece at a speed you can play: an engraved score with a playback cursor, a tempo ladder that climbs after each clean run, and a practice record with a streak.",
  author: { name: "tinys", url: "https://tinys.click" },
  categories: ["music", "education"],
  window: { kind: "standard", resizable: true, fullscreenable: true, width: 940, height: 660 },
  theme: {
    surface: "#5e1a24",
    surfaceDeep: "#43111a",
    onSurface: "#f5e8d4",
    paper: "#fbf6ea",
    ink: "#1d1916",
    muted: "#6c6156",
    rule: "#ddd0b9",
    brass: "#d4a548",
    pencil: "#2b5c8e",
  },
});
