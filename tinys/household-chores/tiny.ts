import { defineTiny } from "@hitslop/tinys";

export default defineTiny({
  slug: "household-chores",
  title: "Household Chores",
  description: "A fridge-door chore wheel for roommates, families and kids: assign chores, tick them off, rotate each week, and keep the points fair.",
  author: { name: "tinys", url: "https://tinys.click" },
  categories: ["personal", "productivity"],
  window: { kind: "standard", width: 460, height: 740 },
  theme: {
    fridge: "#e6ebee",
    fridgeEdge: "#c3ccd2",
    paper: "#fffdf5",
    ink: "#26323a",
    muted: "#66737c",
    rule: "#26323a24",
    accent: "#2b6f9e",
    coral: "#f0705a",
    sky: "#4aa3df",
    mint: "#3fb88a",
    butter: "#f2c230",
    lilac: "#9a7bd6",
    peach: "#f59a5b",
  },
});
