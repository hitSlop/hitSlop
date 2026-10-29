import { defineTiny } from "@hitslop/tinys";

export default defineTiny({
  slug: "resume",
  title: "Resume",
  description: "An editorial paper resume with direct editing, strong typography, and clean export.",
  author: { name: "tinys", url: "https://tinys.click" },
  categories: ["personal", "popular"],
  window: { kind: "standard", width: 680, height: 780 },
  theme: {
    surface: "#fbfaf5",
    paper: "#fbfaf5",
    rail: "#f0eee6",
    ink: "#202725",
    muted: "#66706c",
    accent: "#247a73",
    accentSoft: "#dcebe6",
    danger: "#a03d34",
  },
});
