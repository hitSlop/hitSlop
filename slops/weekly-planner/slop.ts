import { defineSlop } from "hitslop";

export default defineSlop({
  slug: "weekly-planner",
  title: "Weekly Planner",
  description: "A magnetic week board: draw, move, and stretch colorful time blocks to make room for your week.",
  author: { name: "hitSlop", url: "https://hitslop.com" },
  categories: ["productivity", "popular"],
  window: { kind: "standard", width: 880, height: 560 },
  theme: {
    paper: "#f7f9fd",
    paperSoft: "#edf1f8",
    ink: "#172e55",
    muted: "#60708a",
    accent: "#2449a5",
    onAccent: "#ffffff",
    rule: "#dce4f0",
    border: "#ccd7e7",
    danger: "#b43850",
    sky: "#c8e7ff",
    coral: "#ffd2c8",
    mint: "#c6eedb",
    lilac: "#e0d7fc",
  },
});
