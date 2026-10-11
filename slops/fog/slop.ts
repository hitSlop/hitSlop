import { defineSlop } from "hitslop";

export default defineSlop({
  slug: "fog",
  title: "Fog",
  description: "Your webcam as a steamed-up bathroom mirror: breathe to fog it, pinch to draw in it, swipe a fist to wipe it clear.",
  author: { name: "hitSlop", url: "https://hitslop.com" },
  categories: ["creative", "media"],
  window: { kind: "standard", width: 720, height: 540, resizable: true, fullscreenable: true },
  capabilities: ["camera", "microphone"],
  theme: {
    surface: "#e5f2ed",
    room: "#bcd8cc",
    lamp: "#ffffff",
    tile: "#88b7a3",
    frost: "#f0f6f2",
    ink: "#203e34",
    muted: "#526e63",
    onSurface: "#203e34",
    accent: "#ffffff",
    live: "#397a5c",
    alert: "#9c2f25",
  },
});
