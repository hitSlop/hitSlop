import { defineSlop } from "@hitslop/document";
import schema from "./schema";
import variant from "./variant";

export default defineSlop({
  title: variant.title,
  description: "Native window presentation regression fixture.",
  author: { name: "hitSlop" },
  categories: ["productivity"],
  presentation: variant.presentation,
  theme: { accent: "#245ba8" },
  schema,
  initial: { count: 0 },
});
