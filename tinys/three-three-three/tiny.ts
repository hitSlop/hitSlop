import { defineTiny } from "@hitslop/tinys";

export default defineTiny({
  slug: "three-three-three",
  title: "3-3-3 Plan",
  description: "Oliver Burkeman’s daily finite work docket: three hours on one deep project, three urgent tasks, and three maintenance loops.",
  author: { name: "tinys", url: "https://tinys.click" },
  categories: ["productivity", "personal"],
  window: { kind: "standard", width: 560, height: 760 },
  theme: {
    surface: "#f7f6f2",
    paper: "#ffffff",
    ink: "#201f1d",
    muted: "#726e66",
    accent: "#1e3a8a",
    short: "#b45309",
    maintenance: "#0f766e",
    line: "#e6e2d8",
  },
});
