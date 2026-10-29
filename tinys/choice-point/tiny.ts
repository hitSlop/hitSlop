import { defineTiny } from "@hitslop/tinys";

export default defineTiny({
  slug: "choice-point",
  title: "Choice Point",
  description: "Russ Harris’s ACT Choice Point: notice what hooks you, map away and towards moves, and pick one towards move.",
  author: { name: "tinys", url: "https://tinys.click" },
  categories: ["personal", "education"],
  window: { kind: "standard", width: 760, height: 860 },
  theme: {
    surface: "#f5f1e9",
    paper: "#fffdf9",
    ink: "#29241f",
    muted: "#736b62",
    line: "#e4dccf",
    away: "#c9553b",
    towards: "#2b8a66",
  },
});
