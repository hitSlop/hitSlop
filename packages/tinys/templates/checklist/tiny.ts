import { defineTiny } from "@hitslop/tinys";

export default defineTiny({
  slug: "checklist",
  title: "Little Checklist",
  description: "Keep a short list and check things off.",
  author: { name: "tinys" },
  categories: ["productivity"],
  window: { kind: "standard", width: 560, height: 660 },
  theme: { surface: "#faf8f2", ink: "#292f2a", muted: "#686d65", rule: "#dddcd2", accent: "#36694d", panel: "#eceee3" },
});
