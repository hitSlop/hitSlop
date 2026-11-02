import { definePop } from "@hitslop/pop";

export default definePop({
  slug: "doodle-board",
  title: "Doodle Board",
  description: "A little whiteboard for big ideas, pop drawings, and happy accidents.",
  author: { name: "pop", url: "https://dotpop.click" },
  categories: ["creative", "personal"],
  window: { kind: "standard", width: 744, height: 594 },
  theme: {
    frame: "#ef7965",
    board: "#fffcf3",
    tray: "#f9e4cf",
    ink: "#382c2a",
    muted: "#77574c",
    rule: "#b95343",
    focus: "#243e82",
    charcoal: "#343744",
    coral: "#d94839",
    blue: "#2d63b8",
    green: "#238063",
    purple: "#8852a0",
  },
});
