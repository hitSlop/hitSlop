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
  description: "A developer instrument for window clipping, edge input, resizing and independent captures.",
  author: { name: "hitSlop" },
  categories: ["developer-tools"],
  window: variant.window,
  theme: { surface: "#eee9dc", ink: "#19352c", accent: "#dc4c2e", grid: "#d3d0bf", paper: "#fffdf6" },
  document: schema,
  initial: { note: "Type here, then capture.", hits: 0, lastTarget: "None" },
});
