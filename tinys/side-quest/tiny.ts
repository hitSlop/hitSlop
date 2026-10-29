import { defineTiny } from "@hitslop/tinys";

export default defineTiny({
  slug: "side-quest",
  title: "Side Quest",
  description: "Your semester as a doodled notebook map: quests to clear, exam bosses to study down, and stickers to earn.",
  author: { name: "tinys", url: "https://tinys.click" },
  categories: ["games", "education"],
  window: { kind: "standard", width: 940, height: 640 },
  theme: {
    paper: "#fbf7ec",
    grid: "#cfe0ea",
    margin: "#f19a9a",
    ink: "#27304a",
    inkSoft: "#6b7189",
    pen: "#3552c8",
    highlighter: "#f6ef5c",
    boss: "#e0463c",
    spiral: "#9aa0ad",
    pink: "#f7b6cf",
    mint: "#a9e4c8",
    butter: "#f8e08a",
    lilac: "#cdb8f2",
    sky: "#a8d4f5",
    peach: "#fbc39c",
  },
});
