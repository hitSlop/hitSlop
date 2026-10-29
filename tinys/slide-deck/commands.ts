import { defineCommands } from "@hitslop/tinys/commands";
import { s } from "@hitslop/tinys";
import schema, { layouts, themes } from "./schema";
import { addSlide, moveSlide, removeSlide, setLayout, setTheme } from "./deck";

const slide = s.string({ description: "The slide's $id." });

export default defineCommands(schema, {
  addSlide: {
    description: "Append a split-layout slide with placeholder text and return its stable row ID.",
    args: {},
    run: ({ tx }) => addSlide(tx),
  },
  removeSlide: {
    description: "Remove a slide; the last remaining slide cannot be removed.",
    args: { slide },
    run: ({ tx, current }, args) => removeSlide(tx, current, args.slide),
  },
  moveSlide: {
    description: "Move a slide one place earlier (-1) or later (1) in the deck.",
    args: { slide, direction: s.integer({ min: -1, max: 1, description: "-1 moves earlier, 1 moves later." }) },
    run: ({ tx, current }, args) => moveSlide(tx, current, args.slide, args.direction < 0 ? -1 : 1),
  },
  setLayout: {
    description: "Switch a slide to another layout, keeping its text and filling missing fields.",
    args: { slide, layout: s.enum(layouts) },
    run: ({ tx, current }, args) => setLayout(tx, current, args.slide, args.layout),
  },
  setTheme: {
    description: "Set the deck's slide theme.",
    args: { theme: s.enum(themes) },
    run: ({ tx }, args) => setTheme(tx, args.theme),
  },
});
