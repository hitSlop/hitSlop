import { defineSlop } from "hitslop";

export default defineSlop({
  slug: "semester-planner",
  title: "Semester Planner",
  description: "A bird's-eye academic term calendar for mapping syllabus milestones, exam dates, and semester deadlines.",
  author: { name: "hitSlop", url: "https://hitslop.com" },
  categories: ["education", "productivity"],
  window: { kind: "standard", width: 920, height: 580 },
  theme: {
    surface: "#181716",
    paper: "#faf8f3",
    ink: "#1f2328",
    muted: "#57606a",
    accent: "#2563eb",
    line: "#ded8cb",
  },
});
