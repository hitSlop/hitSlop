import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc from "./schema";
import { signifierTypes } from "./signifiers";
import { addEntry, addMonthDay, cycleSignifier, removeEntry, removeMonthDay, setPriority } from "./journal";

export default defineCommands(doc, {
  addEntry: {
    description: "Rapid log an entry at the end of the daily page and return its stable row ID.",
    args: {
      text: s.string({ description: "The entry's text." }),
      type: s.enum(signifierTypes, { description: "The signifier." }),
      star: s.boolean({ description: "True to mark the entry as a priority." }),
    },
    run: ({ tx }, args) => addEntry(tx, args),
  },
  cycleSignifier: {
    description: "Advance an entry to the next signifier in the cycle and return the new signifier.",
    args: { entry: s.string({ description: "The entry's $id." }) },
    run: ({ tx, current }, { entry }) => cycleSignifier(tx, current, entry),
  },
  setPriority: {
    description: "Set or clear an entry's priority star by its $id.",
    args: {
      entry: s.string({ description: "The entry's $id." }),
      star: s.boolean({ description: "True to mark the entry as a priority." }),
    },
    run: ({ tx }, { entry, star }) => setPriority(tx, entry, star),
  },
  removeEntry: {
    description: "Remove an entry from the daily page by its $id.",
    args: { entry: s.string({ description: "The entry's $id." }) },
    run: ({ tx }, { entry }) => removeEntry(tx, entry),
  },
  addMonthDay: {
    description: "Add the next day to the monthly index and return its stable row ID.",
    args: {},
    run: ({ tx, current }) => addMonthDay(tx, current),
  },
  removeMonthDay: {
    description: "Remove a day from the monthly index by its $id.",
    args: { day: s.string({ description: "The monthly day's $id." }) },
    run: ({ tx }, { day }) => removeMonthDay(tx, day),
  },
});
