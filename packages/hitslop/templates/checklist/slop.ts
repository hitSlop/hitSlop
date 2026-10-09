import App from "./App.svelte";
import "hitslop/base.css";
import "./styles.css";
import Icon from "./Icon.svelte";
import * as commands from "./commands";
import { defineSlop } from "hitslop";
import schema from "./schema";

export default defineSlop({
  slug: "checklist",
  view: App,
  icon: Icon,
  commands,
  title: "Little Checklist",
  description: "Keep a short list and check things off.",
  author: { name: "hitSlop" },
  categories: ["productivity"],
  window: { kind: "standard", width: 560, height: 660 },
  theme: { surface: "#faf8f2", ink: "#292f2a", muted: "#686d65", rule: "#dddcd2", accent: "#36694d", panel: "#eceee3" },
  document: schema,
  initial: {
    title: "A little room to think",
    tasks: [
      { text: "Make something small", done: true },
      { text: "Try a change from the terminal" },
      { text: "Close this list and open it again" },
    ],
  },
});
