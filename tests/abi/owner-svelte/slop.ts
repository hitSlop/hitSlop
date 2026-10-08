import App from "./App.svelte";
import "./styles.css";
import Export from "./Export.svelte";
import Icon from "./Icon.svelte";
import { defineSlop } from "hitslop";
import schema from "./schema";
import { bump, decline } from "./actions";

export default defineSlop({
  slug: "owner-svelte",
  view: App,
  export: Export,
  icon: Icon,
  title: "ABI Svelte consumer",
  description: "Frozen Svelte adapter consumer of the runtime ABI.",
  author: { name: "hitSlop" },
  categories: ["utilities"],
  window: { kind: "standard", width: 480, height: 480 },
  theme: { accent: "#335577" },
  document: schema,
  commands: { bump, decline },
  initial: {
    title: "Svelte ABI 2",
    version: "Authored version",
    epoch: 42,
    done: false,
    hits: 0,
    rows: [],
    label: "",
    ratio: 0.5,
    count: 0,
    lane: "todo",
    settings: { volume: 5, muted: false },
    colors: [],
    checkins: {},
    cells: {},
    slides: [{ title: "Nested composition", blocks: [{ text: "First block" }, { text: "Second block" }] }],
  },
});
