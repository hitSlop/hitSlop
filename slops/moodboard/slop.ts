import { defineSlop } from "hitslop";

export default defineSlop({
  slug: "moodboard",
  title: "Moodboard",
  description: "Drop in images and fly through them in a soft 3D tunnel; caption each one and keep the whole board in one document.",
  author: { name: "hitSlop", url: "https://hitslop.com" },
  categories: ["creative"],
  window: { kind: "standard", width: 560, height: 720 },
  theme: {
    night: "#0e0d12",
    glow: "#2a2140",
    paper: "#f2eee6",
    muted: "#9a94aa",
    accent: "#ff6b4a",
    onAccent: "#1a0f0c",
    card: "#1a1722",
    rule: "#ffffff1f",
  },
});
