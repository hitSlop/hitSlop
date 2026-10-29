import { defineTiny } from "@hitslop/tinys";
import variant from "./variant";

export default defineTiny({
  slug: variant.slug,
  title: variant.title,
  description: "Native window presentation regression fixture.",
  author: { name: "tinys" },
  categories: ["productivity"],
  window: { ...variant.window, fullscreenable: true },
  theme: { accent: "#245ba8" },
});
