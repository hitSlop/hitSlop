import { defineSlop } from "hitslop";

export default defineSlop({
  slug: "daily-frame",
  title: "Daily Frame",
  description: "One photo a day from your camera, kept as a stack of prints and played back as a flipbook. For a plant, a drawing, a workout or a growing baby.",
  author: { name: "hitSlop", url: "https://hitslop.com" },
  categories: ["creative", "personal"],
  window: { kind: "standard", width: 540, height: 780, resizable: true },
  capabilities: ["camera"],
  theme: {
    body: "#1f1d1b",
    panel: "#2b2926",
    film: "#121110",
    print: "#f5f1e8",
    ink: "#2a2622",
    inkMuted: "#7a7166",
    onBody: "#eee8dc",
    muted: "#a69d8f",
    stamp: "#ff8a24",
    live: "#ff4f3f",
    accent: "#ffbe55",
    blank: "#5b5249",
  },
});
