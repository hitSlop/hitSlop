import { defineSlop } from "hitslop";

export default defineSlop({
  slug: "pros-cons-sheet",
  title: "Decision Balance",
  description: "Weigh the reasons for and against a decision, adjust their importance, and record your own conclusion.",
  author: { name: "hitSlop", url: "https://hitslop.com" },
  categories: ["productivity", "personal"],
  window: { kind: "standard", width: 720, height: 680 },
  theme: {
    onSurface: "#f5f2e9",
    mutedOnSurface: "#c4c9bc",
    surfacePro: "#c0d5b8",
    surfaceCon: "#f1bca7",
    surfaceFocus: "#e4ca8b",
    surface: "#343935",
    paper: "#f5f2e9",
    paperSoft: "#e8e6db",
    ink: "#29332d",
    muted: "#62675d",
    pro: "#21564a",
    proSoft: "#e4eee8",
    con: "#8a3324",
    conSoft: "#f3e6e0",
    brass: "#8b937c",
    brassSoft: "#dbdecf",
    danger: "#8a3324",
  },
});
