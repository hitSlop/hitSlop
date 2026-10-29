import { defineTiny } from "@hitslop/tinys";

export default defineTiny({
  slug: "capture-notebook",
  title: "Capture Notebook",
  description: "A field notebook with a camera and a tape recorder built in: take photos, record audio and jot notes, all kept in the document.",
  author: { name: "tinys", url: "https://tinys.click" },
  categories: ["media", "personal"],
  window: { kind: "standard", resizable: true, width: 520, height: 700 },
  capabilities: ["camera", "microphone"],
  theme: {
    body: "#22261f",
    bodyInk: "#ebe6d6",
    bodyMuted: "#9fa08f",
    lens: "#0e100d",
    paper: "#f6f1e5",
    ink: "#262a22",
    muted: "#686a5e",
    rule: "#ddd4bf",
    tape: "#e4d4a8",
    shutter: "#f0b43c",
    onShutter: "#2a2210",
    rec: "#e2412e",
    live: "#7fcf8a",
  },
});
