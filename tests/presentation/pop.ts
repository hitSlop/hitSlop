import { definePop } from "@hitslop/pop";
import variant from "./variant";

export default definePop({
  slug: variant.slug,
  title: variant.title,
  description: "Native window presentation regression fixture.",
  author: { name: "pop" },
  categories: ["productivity"],
  window: { ...variant.window, fullscreenable: true },
  theme: { accent: "#245ba8" },
});
