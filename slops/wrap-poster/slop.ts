import { defineSlop } from "hitslop";

export default defineSlop({
  slug: "wrap-poster",
  title: "Wrap Poster",
  description: "An editorial poster where your words flow around shapes you drag, line by line, so text and layout always fit together.",
  author: { name: "hitSlop", url: "https://hitslop.com" },
  categories: ["creative"],
  window: { kind: "standard", width: 560, height: 780 },
  theme: {
    desk: "#2b2622",
    paper: "#f6efe1",
    ink: "#1f1b16",
    tomato: "#e8553d",
    cobalt: "#2f4fd8",
    mustard: "#f0b429",
    glass: "#fffaf0f2",
    accent: "#1f1b16",
    onAccent: "#f6efe1",
  },
});
