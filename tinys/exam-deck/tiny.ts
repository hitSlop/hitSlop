import { defineTiny } from "@hitslop/tinys";

export default defineTiny({
  slug: "exam-deck",
  title: "Exam Deck",
  description: "Flashcards scheduled to get you ready by exam day: today's cards, a two-week load forecast and how ready you'll be.",
  author: { name: "tinys", url: "https://tinys.click" },
  categories: ["education", "productivity"],
  window: { kind: "standard", resizable: true, fullscreenable: true, width: 520, height: 720 },
  theme: {
    paper: "#fbf7ec",
    fiber: "#efe7d4",
    form: "#b4472f",
    tint: "#f3dcd2",
    graphite: "#2c2d31",
    pencil: "#66676c",
    timing: "#141414",
  },
});
