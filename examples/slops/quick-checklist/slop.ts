import { defineSlop } from "@hitslop/document";
import schema from "./schema";

export default defineSlop({
  title: "Quick Checklist",
  description: "A blush pocket utility for capturing, finishing, and filing short task lists.",
  author: { name: "hitSlop", url: "https://hitslop.com" },
  categories: ["productivity", "personal"],
  presentation: { width: 480, height: 620 },
  theme: {
    surface: "#e98996",
    paper: "#fff9f3",
    ink: "#432830",
    muted: "#82636a",
    accent: "#a43d59",
    onAccent: "#432830",
    rule: "#ead9cb",
    control: "#f1dfc9",
    highlight: "#f8d65a",
    success: "#94d7c0",
    successBorder: "#6c354b",
    filed: "#fff5e9",
    controlBorder: "#d8c0af",
  },
  schema,
  initial: {
    title: "Little things, today",
    tasks: [
      { text: "Send the first draft", done: true, archived: false },
      { text: "Take a walk without my phone", done: false, archived: false },
      { text: "Make a little room for the weekend", done: false, archived: false },
    ],
  },
});
