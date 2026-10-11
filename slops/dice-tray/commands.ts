import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc from "./schema";
import { clearHistory, rename, roll, rollAgain, rollFormula, setFormula, setModifier } from "./dice";

export default defineCommands(doc, {
  roll: {
    description: "Roll the dice pool, or first make the given formula (like '2d6+3' or '1d20+5') the pool, save the roll to the history and return its $id, total and values.",
    args: { formula: s.optional(s.string({ description: "Dice such as '1d8 + 3d6 + 4'; d4, d6, d8, d10, d12 and d20 only." })) },
    run: ({ tx, current, random, now }, { formula }) =>
      formula === undefined ? roll(tx, current, random, now) : rollFormula(tx, current, formula, random, now),
  },
  rollAgain: {
    description: "Roll an earlier roll's dice and modifier again (they become the pool) and return the new roll's $id, total and values.",
    args: { roll: s.string({ description: "The earlier roll's $id." }) },
    run: ({ tx, current, random, now }, args) => rollAgain(tx, current, args.roll, random, now),
  },
  setPool: {
    description: "Replace the dice pool and modifier with a formula like '2d6+3' and return it written out.",
    args: { formula: s.string() },
    run: ({ tx }, { formula }) => setFormula(tx, formula),
  },
  setModifier: {
    description: "Set the flat modifier added to every roll, from -99 to 99.",
    args: { modifier: s.integer() },
    run: ({ tx }, { modifier }) => setModifier(tx, modifier),
  },
  clearHistory: {
    description: "Remove every saved roll and return how many were removed.",
    args: {},
    run: ({ tx, current }) => clearHistory(tx, current),
  },
  rename: {
    description: "Rename the tray (up to 40 characters), usually after the table or game.",
    args: { title: s.string() },
    run: ({ tx }, { title }) => rename(tx, title),
  },
});
