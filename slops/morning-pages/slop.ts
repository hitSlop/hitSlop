import { defineSlop } from "hitslop";

export default defineSlop({
  slug: "morning-pages",
  title: "Morning Pages",
  description: "Julia Cameron’s three-page morning brain clearing: a distraction-free 750-word stream-of-consciousness feed with a live word odometer.",
  author: { name: "hitSlop", url: "https://hitslop.com" },
  categories: ["personal", "productivity"],
  window: { kind: "standard", width: 640, height: 820 },
  theme: {
    paper: "#f6e59b",
    stub: "#ead37a",
    ink: "#2c2618",
    muted: "#7a6d4e",
    dim: "#a39470",
    rule: "#9eb8d4",
    margin: "#d24a4a",
    gold: "#9a4a0a",
    goldSoft: "#f3e6b8",
    hole: "#c9b56a",
  },
});
