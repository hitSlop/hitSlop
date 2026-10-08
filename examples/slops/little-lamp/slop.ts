import { defineSlop } from "hitslop";
import "hitslop/base.css";
import "./styles.css";
import App from "./App.svelte";
import Export from "./Export.svelte";
import Icon from "./Icon.svelte";
import document from "./schema";

export default defineSlop({
  slug: "little-lamp",
  title: "Little Lamp",
  description: "A little desktop companion. Tap to wake it; move your pointer and it looks back.",
  author: { name: "hitSlop", url: "https://hitslop.com" },
  categories: ["personal"],
  window: {
    kind: "standard", width: 320, height: 320, background: "transparent",
    shape: { viewBox: [320, 320], path: "M160 20 C80 20 55 75 52 155 L40 218 L8 292 Q160 336 312 292 L280 218 L268 155 C265 75 240 20 160 20Z" },
  },
  theme: { accent: "#f3c76d" },
  document,
  initial: {},
  view: App,
  export: Export,
  icon: Icon,
});
