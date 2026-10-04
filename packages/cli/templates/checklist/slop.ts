import { defineSlop } from "@hitslop/document";
import schema from "./schema";

export default defineSlop({
  title: "Little Checklist",
  description: "Keep a short list and check things off.",
  author: { name: "hitSlop" },
  categories: ["productivity"],
  presentation: { width: 560, height: 660 },
  theme: { surface: "#faf8f2", ink: "#292f2a", muted: "#686d65", rule: "#dddcd2", accent: "#36694d", panel: "#eceee3" },
  schema,
  initial: {
    title: "A little room to think",
    tasks: [
      { text: "Make something small", done: true },
      { text: "Try a change from the terminal", done: false },
      { text: "Close this list and open it again", done: false },
    ],
  },
});
