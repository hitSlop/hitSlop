import { defineTiny } from "@hitslop/tinys";

export default defineTiny({
  slug: "harada-method",
  title: "Harada Method",
  description: "The Open Window 64 chart: one core goal, eight themes, and sixty-four concrete actions on a single hand-ruled sheet.",
  author: { name: "tinys", url: "https://tinys.click" },
  categories: ["productivity", "education"],
  window: { kind: "standard", width: 800, height: 880 },
  theme: {
    field: "#eceef4",
    sheet: "#ffffff",
    sheetShade: "#e3e7f1",
    ink: "#14161e",
    inkSoft: "#5f6478",
    hairline: "#d2d7e3",
    accent: "#4c3bd6",
    accentDeep: "#33269f",
    onAccent: "#f2f1ff",
    danger: "#b3261e",
    placeholder: "#9aa0b4",
  },
});
