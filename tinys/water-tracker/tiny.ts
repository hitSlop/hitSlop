import { defineTiny } from "@hitslop/tinys";

export default defineTiny({
  slug: "water-tracker",
  title: "Water Tracker",
  description: "A translucent aqua hydration instrument with a live liquid-level readout and quick-tap logging.",
  author: { name: "tinys", url: "https://tinys.click" },
  categories: ["health", "utilities"],
  window: { kind: "standard", width: 420, height: 580 },
  theme: {
    surface: "#08212b",
    panel: "#0e3240",
    chamber: "#041219",
    ink: "#f0fdfa",
    muted: "#a5f3fc",
    accent: "#22d3ee",
    accentDeep: "#0891b2",
    border: "#164e63",
    glow: "#22d3ee66",
    celebrate: "#f59e0b",
    danger: "#ef4444",
  },
});
