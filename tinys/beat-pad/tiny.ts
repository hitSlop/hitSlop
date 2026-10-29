import { defineTiny } from "@hitslop/tinys";

export default defineTiny({
  slug: "beat-pad",
  title: "Beat Pad",
  description: "A 16-step drum and bass sequencer: tap in a beat, pick a kit, set the tempo and swing, and your pattern is saved with the document.",
  author: { name: "tinys", url: "https://tinys.click" },
  categories: ["music"],
  window: { kind: "standard", width: 560, height: 700 },
  theme: {
    studio: "#14121c",
    panel: "#1f1b2c",
    cellOff: "#2b2640",
    text: "#f3f0ff",
    muted: "#9d96b8",
    kick: "#ff5a5f",
    snare: "#ffd23f",
    hat: "#4ecdc4",
    bass: "#9b6bff",
    onTrack: "#14121c",
  },
});
