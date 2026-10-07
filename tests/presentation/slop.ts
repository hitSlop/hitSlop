import App from "./App.svelte";
import "./styles.css";
import Export from "./Export.svelte";
import Icon from "./Icon.svelte";
import { defineSlop } from "hitslop";
import schema from "./schema";
import variant from "./variant";

export default defineSlop({
  slug: variant.slug,
  view: App,
  export: Export,
  icon: Icon,
  title: variant.title,
  description: "Native window presentation regression fixture.",
  author: { name: "hitSlop" },
  categories: ["productivity"],
  window: variant.window,
  theme: { accent: "#245ba8" },
  document: schema,
  initial: { count: 0 },
});
