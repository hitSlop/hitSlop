import { defineTiny } from "@hitslop/tinys";

export default defineTiny({
  slug: "daily-planner",
  title: "Daily Planner",
  description: "A slim day board with three priorities, movable time blocks, and a live now marker.",
  author: { name: "tinys", url: "https://tinys.click" },
  categories: ["productivity", "personal"],
  window: { kind: "standard", width: 400, height: 900 },
  theme: {
    surface: "#f9fcfb",
    panel: "#eef5f2",
    ink: "#183d3b",
    muted: "#617875",
    teal: "#164e4a",
    tealLight: "#cce7e1",
    accent: "#ffb17e",
    now: "#c34e22",
    mint: "#dff1d8",
    lavender: "#e9e2f8",
    rule: "#d8e5df",
    onAccent: "#ffffff",
    danger: "#a83137",
  },
});
