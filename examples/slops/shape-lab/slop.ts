import { defineSlop } from "@hitslop/document";
import schema from "./schema";
import variant from "./variant";

export default defineSlop({
  title: variant.title,
  description: "A developer instrument for window clipping, edge input, resizing and independent captures.",
  author: { name: "hitSlop" },
  categories: ["developer-tools"],
  presentation: variant.presentation,
  theme: { surface: "#eee9dc", ink: "#19352c", accent: "#dc4c2e", grid: "#d3d0bf", paper: "#fffdf6" },
  schema,
  initial: { note: "Type here, then capture.", hits: 0, lastTarget: "None" },
});
