import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc, { categories } from "./schema";
import { addItem, clearDone, removeItem, setDone } from "./items";

export default defineCommands(doc, {
  addItem: {
    description: "Add an unchecked item to an aisle and return its stable row ID.",
    args: {
      text: s.string({ description: "The item's text." }),
      category: s.enum(categories, { description: "The aisle." }),
    },
    run: ({ tx }, { text, category }) => addItem(tx, text, category),
  },
  setDone: {
    description: "Check or uncheck an item by its $id.",
    args: {
      item: s.string({ description: "The item's $id." }),
      done: s.boolean({ description: "True when the item is in the cart." }),
    },
    run: ({ tx }, { item, done }) => setDone(tx, item, done),
  },
  removeItem: {
    description: "Remove an item from the pad by its $id.",
    args: { item: s.string({ description: "The item's $id." }) },
    run: ({ tx }, { item }) => removeItem(tx, item),
  },
  clearDone: {
    description: "Remove every checked item and return the number removed.",
    args: {},
    run: ({ tx, current }) => clearDone(tx, current),
  },
});
