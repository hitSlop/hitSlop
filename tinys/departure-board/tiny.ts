import { defineTiny } from "@hitslop/tinys";

export default defineTiny({
  slug: "departure-board",
  title: "Departure Board",
  description: "Your day as a split-flap airport board: set the stops, tap a status to flip it, and watch every letter clack into place.",
  author: { name: "tinys", url: "https://tinys.click" },
  categories: ["personal", "utilities"],
  window: { kind: "standard", width: 560, height: 640 },
  theme: {
    board: "#101216",
    panel: "#181a20",
    flap: "#23262e",
    flapDark: "#1b1d24",
    seam: "#050608",
    text: "#f3e9c6",
    dim: "#8d909a",
    amber: "#ffb81c",
    go: "#5ee08f",
    warn: "#ff6a57",
    paper: "#f1ecdd",
    paperInk: "#1b1d22",
    paperMuted: "#6a6d75",
    rule: "#1b1d2233",
  },
});
