import { defineSlop } from "@hitslop/document";
import schema from "./schema";

export default defineSlop({
  title: "ABI Svelte consumer",
  description: "Frozen Svelte adapter consumer of the runtime ABI.",
  author: { name: "hitSlop" },
  categories: ["utilities"],
  presentation: { width: 480, height: 480 },
  theme: { accent: "#335577" },
  schema,
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
  },
});
