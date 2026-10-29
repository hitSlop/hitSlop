import { defineTiny } from "@hitslop/tinys";

export default defineTiny({
  slug: "soma-amp",
  title: "SomaAmp",
  description: "A classic skin-compatible desktop receiver for eight listener-supported SomaFM stations.",
  author: { name: "tinys", url: "https://tinys.click" },
  categories: ["music", "media"],
  // Resizable so VIS can widen the window for MilkDrop: the host refuses resize requests
  // from a fixed-size window.
  window: { kind: "standard", width: 275, height: 470, resizable: true },
  theme: {
    surface: "#080a0b",
    panel: "#111416",
    ink: "#b7c0c6",
    muted: "#818b91",
    accent: "#43db7f",
    danger: "#ff7b72",
    border: "#30363a",
  },
});
