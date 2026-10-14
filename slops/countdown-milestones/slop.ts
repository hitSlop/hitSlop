import { defineSlop } from "hitslop";

export default defineSlop({
  slug: "countdown-milestones",
  title: "Countdown & Milestones",
  description: "A playful desk-calendar countdown with editable milestones and a date worth remembering.",
  author: { name: "hitSlop", url: "https://hitslop.com" },
  categories: ["utilities", "productivity"],
  window: { kind: "standard", width: 440, height: 580 },
  widget: { refreshMinutes: 30 },
  theme: {
    paper: "#b9dcff",
    page: "#edf6ff",
    ink: "#25212c",
    muted: "#4d5e70",
    coral: "#f45d4f",
    yellow: "#f2d94e",
    rule: "#25212c2b",
    focus: "#a82924",
  },
});
