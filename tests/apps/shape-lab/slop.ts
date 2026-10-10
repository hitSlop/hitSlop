import { defineSlop } from "hitslop";
import variant from "./variant";

export default defineSlop({
  slug: variant.slug,
  title: variant.title,
  description: "A developer instrument for window clipping, edge input, resizing and independent captures.",
  author: { name: "hitSlop" },
  categories: ["developer-tools"],
  window: variant.window,
  theme: { surface: "#eee9dc", ink: "#19352c", accent: "#dc4c2e", grid: "#d3d0bf", paper: "#fffdf6" },
});
