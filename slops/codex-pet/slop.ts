import { defineSlop } from "hitslop";

export default defineSlop({
  slug: "codex-pet",
  title: "Codex Pet",
  description: "A transparent desktop companion that animates Codex pet packages and accepts local ZIP imports.",
  author: { name: "hitSlop", url: "https://hitslop.com" },
  categories: ["personal", "developer-tools"],
  window: { kind: "standard", width: 240, height: 180, resizable: false, background: "transparent" },
  theme: {
    surface: "#00000000",
    ink: "#fff7eb",
    accent: "#ffd281",
    plate: "#3b2821",
    rail: "#231b18c7",
  },
});
