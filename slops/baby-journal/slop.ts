import { defineSlop } from "hitslop";

export default defineSlop({
  slug: "baby-journal",
  title: "Little Days",
  description: "A gentle baby journal for feedings, diaper changes, sleep, and little memories, with a name beside every act of care.",
  author: { name: "hitSlop", url: "https://hitslop.com" },
  categories: ["personal", "popular"],
  window: { kind: "standard", resizable: true, width: 900, height: 640 },
  theme: {
    surface: "#fffaf0",
    paper: "#fffdf8",
    ink: "#29263f",
    muted: "#736b79",
    rule: "#eadfce",
    peach: "#ffc1a6",
    pink: "#f8c3d3",
    blue: "#bedcec",
    lilac: "#dcccf7",
    mint: "#c6e3d4",
    yellow: "#ffe294",
    apricot: "#f9c889",
    feeding: "#ffc1a6",
    diaper: "#ffe294",
    sleep: "#dcccf7",
    note: "#c6e3d4",
  },
});
