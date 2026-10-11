import { defineSlop } from "hitslop";

export default defineSlop({
  slug: "minesweeper",
  title: "Buoy Sweep",
  description:
    "Minesweeper on a harbor chart: clear the water, flag the mines with buoys, and beat your best time. Your game resumes where you left it.",
  author: { name: "hitSlop", url: "https://hitslop.com" },
  categories: ["games"],
  window: { kind: "standard", width: 400, height: 560 },
  theme: {
    sea: "#0f3b4d",
    chart: "#efe7cf",
    water: "#bcd8d4",
    waterHit: "#d9ebe7",
    ink: "#12263a",
    muted: "#5b6f7a",
    buoy: "#e8552f",
    onBuoy: "#fff7ea",
    gold: "#e0a526",
  },
});
