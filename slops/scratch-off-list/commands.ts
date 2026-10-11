import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc from "./schema";
import { addItem, recoverAll, removeItem, revealItem, setDone } from "./items";

export default defineCommands(doc, {
  addItem: {
    description: "Add an item to the card, already revealed, and return its stable row ID.",
    args: { text: s.string({ description: "What to try." }) },
    run: ({ tx }, { text }) => addItem(tx, text),
  },
  revealItem: {
    description: "Scratch a covered item open by its $id.",
    args: { item: s.string({ description: "The item's $id." }) },
    run: ({ tx }, { item }) => revealItem(tx, item),
  },
  setDone: {
    description: "Stamp an item done or clear the stamp, by its $id.",
    args: {
      item: s.string({ description: "The item's $id." }),
      done: s.boolean({ description: "True to stamp the item done." }),
    },
    run: ({ tx }, { item, done }) => setDone(tx, item, done),
  },
  removeItem: {
    description: "Remove an item from the card by its $id.",
    args: { item: s.string({ description: "The item's $id." }) },
    run: ({ tx }, { item }) => removeItem(tx, item),
  },
  recoverAll: {
    description: "Cover every item again and clear its stamp; return how many were revealed.",
    args: {},
    run: ({ tx, current }) => recoverAll(tx, current),
  },
});
