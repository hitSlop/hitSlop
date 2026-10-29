import { defineTiny } from "@hitslop/tinys";

export default defineTiny({
  slug: "ivy-lee-method",
  title: "Ivy Lee Method",
  description: "A strict six-item single-tasking ledger: work item 1 until finished, never touch item 2 prematurely, and roll over incomplete work at sundown.",
  author: { name: "tinys", url: "https://tinys.click" },
  categories: ["productivity", "personal"],
  window: { kind: "standard", width: 460, height: 680 },
  theme: {
    surface: "#f9f8f5",
    paper: "#ffffff",
    ink: "#1f1e1c",
    muted: "#757067",
    accent: "#b45309",
    line: "#e8e4dc",
  },
});
