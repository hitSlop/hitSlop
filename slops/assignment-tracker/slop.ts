import { defineSlop } from "hitslop";

export default defineSlop({
  slug: "assignment-tracker",
  title: "Assignment Tracker",
  description: "A plum study pad for assignments, deadlines, course notes, and the satisfaction of checking work off.",
  author: { name: "hitSlop", url: "https://hitslop.com" },
  categories: ["education", "popular"],
  window: { kind: "standard", width: 620, height: 700 },
  theme: {
    surface: "#ffffff",
    paper: "#ffffff",
    panel: "#f6f2fa",
    ink: "#32223f",
    muted: "#776781",
    rule: "#e9e0ee",
    plum: "#533163",
    plumDeep: "#3e224c",
    lilac: "#e5d6f4",
    accent: "#a64038",
    coral: "#ffc1ae",
    mint: "#c5e9d9",
    done: "#326554",
    onAccent: "#ffffff",
    danger: "#a43e42",
  },
});
