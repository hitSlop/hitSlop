import App from "./App.svelte";
import "./styles.css";
import Export from "./Export.svelte";
import Icon from "./Icon.svelte";
import * as commands from "./commands";
import { defineSlop } from "hitslop";
import { height, silhouette, width } from "./glass";
import schema from "./schema";

export default defineSlop({
  slug: "hourglass",
  view: App,
  export: Export,
  icon: Icon,
  commands,
  title: "Hourglass",
  description: "A frosted sand glass that drains toward a time you choose.",
  author: { name: "hitSlop", url: "https://hitslop.com" },
  categories: ["productivity", "personal"],
  window: { kind: "standard",
    width,
    height,
    shape: { path: silhouette, viewBox: [width, height] },
    lockAspect: true,
    background: "glass",
  },
  theme: {
    glass: "#fff8ec5c",
    sand: "#e2b15e",
    frame: "#4b2f1f",
    onFrame: "#f4e7d2",
    ink: "#2a1f17",
  },
  document: schema,
  initial: { title: "", start: 0, end: 0 },
});
