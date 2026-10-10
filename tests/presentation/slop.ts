import { defineSlop } from "hitslop";
import variant from "./variant";

export default defineSlop({
  slug: variant.slug,
  title: variant.title,
  description: "Native window presentation regression fixture.",
  author: { name: "hitSlop" },
  categories: ["productivity"],
  window: { ...variant.window, fullscreenable: true },
  theme: { accent: "#245ba8" },
});
