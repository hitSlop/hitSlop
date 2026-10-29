import { defineCommands } from "@hitslop/tinys/commands";
import { s } from "@hitslop/tinys";
import schema, { slots } from "./schema";
import { addOption, removeOption, setOption, setTitle, spin } from "./options";

export default defineCommands(schema, {
  setTitle: {
    description: "Set whose future the page is about; the title is shown on the future card.",
    args: { title: s.string({ description: "The name, or blank for none." }) },
    run: ({ tx }, { title }) => setTitle(tx, title),
  },
  addOption: {
    description: "Add an empty option to a block (at most 6 per block) and return its stable row ID.",
    args: { slot: s.enum(slots, { description: "The block." }) },
    run: ({ tx, current }, { slot }) => addOption(tx, slot, current),
  },
  setOption: {
    description: "Replace an option's text by its $id; blank options are left out of the game.",
    args: {
      option: s.string({ description: "The option's $id." }),
      text: s.string({ description: "The option's new text." }),
    },
    run: ({ tx }, { option, text }) => setOption(tx, option, text),
  },
  removeOption: {
    description: "Remove an option by its $id.",
    args: { option: s.string({ description: "The option's $id." }) },
    run: ({ tx }, { option }) => removeOption(tx, option),
  },
  spin: {
    description: "Draw a spiral of 2 to 9 loops from the command's random source, store the count and return it.",
    args: {},
    run: ({ tx, random }) => spin(tx, random),
  },
});
