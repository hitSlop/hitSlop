import { defineTiny } from "@hitslop/tinys";

export default defineTiny({
  slug: "dice-tray",
  title: "Dice Tray",
  description: "A walnut dice tray for tabletop games: pick your dice and a modifier, roll, and read the total with a short history of what came up.",
  author: { name: "tinys", url: "https://tinys.click" },
  categories: ["games", "utilities"],
  window: { kind: "standard", width: 460, height: 700, fullscreenable: true },
  theme: {
    wood: "#7a4a2c",
    woodDark: "#3f2418",
    felt: "#6d1e2c",
    feltDeep: "#3a0e18",
    die: "#f4ecdb",
    pip: "#2a1812",
    brass: "#d9ad55",
    paper: "#f6eedd",
    ink: "#2b1d15",
    muted: "#76604f",
    rule: "#e2d3b8",
  },
});
