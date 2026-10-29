import { defineTiny } from "@hitslop/tinys";

export default defineTiny({
  slug: "grade-calculator",
  title: "Grade Calculator",
  description: "An academic gradebook and GPA forecaster with a 'What do I need on the final?' target solver.",
  author: { name: "tinys", url: "https://tinys.click" },
  categories: ["education", "utilities"],
  window: { kind: "standard", width: 680, height: 520 },
  theme: {
    surface: "#171615",
    paper: "#fdfbf7",
    ink: "#1f2328",
    muted: "#57606a",
    accent: "#2b593f",
    line: "#ded7c7",
  },
});
