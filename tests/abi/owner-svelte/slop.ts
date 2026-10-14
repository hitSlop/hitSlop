import { defineSlop } from "hitslop";
import variant from "./variant";

export default defineSlop({
  slug: "owner-svelte",
  title: "ABI Svelte consumer",
  description: "Frozen Svelte adapter consumer of the runtime ABI.",
  author: { name: "hitSlop" },
  categories: ["utilities"],
  window: { kind: "standard", width: 480, height: 480 },
  theme: { accent: "#335577", ink: "#112233", "paper-tint": "#eef4ffaa" },
  widget: { refreshMinutes: 60 },
  ...variant,
});
