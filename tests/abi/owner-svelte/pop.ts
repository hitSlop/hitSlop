import { definePop } from "@hitslop/pop";
import variant from "./variant";

export default definePop({
  slug: "owner-svelte",
  title: "ABI Svelte consumer",
  description: "Frozen Svelte adapter consumer of the runtime ABI.",
  author: { name: "pop" },
  categories: ["utilities"],
  window: { kind: "standard", width: 480, height: 480 },
  theme: { accent: "#335577", ink: "#112233", "paper-tint": "#eef4ffaa" },
  widget: { refreshMinutes: 60 },
  ...variant,
});
