import { defineTiny } from "@hitslop/tinys";

export default defineTiny({
  slug: "quiet-meter",
  title: "Quiet Meter",
  description: "A desk-top traffic light that listens to a room and shows when it is too loud. Nothing is recorded.",
  author: { name: "tinys", url: "https://tinys.click" },
  categories: ["utilities", "education"],
  window: { kind: "standard", width: 420, height: 660 },
  capabilities: ["microphone"],
  theme: {
    surface: "#d8d3c6",
    panel: "#f1ede2",
    housing: "#23272c",
    housingEdge: "#3a4047",
    onHousing: "#eef0ee",
    ink: "#262a2e",
    muted: "#625f58",
    rule: "#c6c0b0",
    quiet: "#46c47c",
    warn: "#f0b429",
    loud: "#ee5a45",
    focus: "#1f6feb",
  },
});
