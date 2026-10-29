import { defineTiny } from "@hitslop/tinys";

export default defineTiny({
  slug: "metronome-tapper",
  title: "Metronome & BPM",
  description: "A precision studio metronome with an animated pendulum, synthesized woodblock clicks, tap tempo, and time signatures.",
  author: { name: "tinys", url: "https://tinys.click" },
  categories: ["music", "utilities"],
  window: { kind: "standard", width: 430, height: 600 },
  theme: {
    chassis: "#1a1614",
    chassisHi: "#3d342e",
    chassisRim: "#2d2622",
    brass: "#d4a337",
    brassHi: "#f5d17a",
    brassShadow: "#8f681a",
    screen: "#0c0a09",
    glow: "#f59e0b",
    glowAccent: "#ef4444",
    ink: "#fdfbf7",
    inkMuted: "#9a9086",
    focus: "#f59e0b",
  },
});
