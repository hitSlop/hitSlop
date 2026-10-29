import { defineTiny } from "@hitslop/tinys";

export default defineTiny({
  slug: "habit-heatmap",
  title: "Habit Heatmap",
  description: "Keep a twelve-week record of small daily habits, one square at a time.",
  author: { name: "tinys", url: "https://tinys.click" },
  categories: ["personal", "productivity"],
  window: { kind: "standard", width: 620, height: 480 },
  widget: { refreshMinutes: 30 },
  theme: {
    shell: "#332436", paper: "#eee8dd", ink: "#29232b", muted: "#6c616d",
    mint: "#a8d3b5", coral: "#f26a5a", butter: "#eee487", blue: "#a9d2df",
    lavender: "#cfb5e5", rose: "#e7a8bd", peach: "#f2bb94", sage: "#b8c59a",
    teal: "#83c7c1", periwinkle: "#acb9e7", amber: "#e5bf69", stone: "#c6b9aa",
    rule: "#d2c9cd", focus: "#963d32",
  },
});
