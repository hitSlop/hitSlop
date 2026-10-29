import { defineTiny } from "@hitslop/tinys";

export default defineTiny({
  slug: "bullet-journal",
  title: "Bullet Journal",
  description: "Ryder Carroll’s analog rapid logging on cream dot-grid paper: tasks, events, notes, and classic signifier transitions.",
  author: { name: "tinys", url: "https://tinys.click" },
  categories: ["productivity", "personal"],
  window: { kind: "standard", width: 600, height: 820 },
  theme: {
    surface: "#fcfbf7",
    ink: "#22201d",
    muted: "#746f66",
    dim: "#aaa497",
    dot: "#dcd7cc",
    rule: "#cfc9bd",
    border: "#e6e1d6",
    accent: "#b45309",
    onAccent: "#fcfbf7",
    scheduled: "#4338ca",
    control: "#f7f4ee",
    ribbon: "#9f1239",
    danger: "#b91c1c",
  },
});
