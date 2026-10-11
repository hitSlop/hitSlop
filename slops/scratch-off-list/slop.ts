import { defineSlop } from "hitslop";

export default defineSlop({
  slug: "scratch-off-list",
  title: "Scratch-Off List",
  description: "A lottery-ticket bucket list: scratch the silver foil to reveal your next adventure, then stamp it done and share the card.",
  author: { name: "hitSlop", url: "https://hitslop.com" },
  categories: ["games", "personal"],
  window: { kind: "standard", width: 480, height: 720 },
  theme: {
    night: "#2a0f3a",
    ticket: "#fff3d6",
    ink: "#3a0f4a",
    muted: "#7d5d88",
    band: "#d81b72",
    onBand: "#fff7e0",
    gold: "#f2b632",
    foil: "#cfd2da",
    foilDark: "#8f94a3",
    done: "#14915a",
    rule: "#3a0f4a2e",
  },
});
