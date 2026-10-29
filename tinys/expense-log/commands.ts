import { defineCommands } from "@hitslop/tinys/commands";
import { s } from "@hitslop/tinys";
import schema, { categories } from "./schema";
import { addItem, removeItem, setCurrency } from "./items";

export default defineCommands(schema, {
  addItem: {
    description: "Record a purchase stamped with the current time and return its stable row ID.",
    args: {
      title: s.string({ description: "What was bought." }),
      amount: s.number({ description: "The amount, above zero; stored rounded to cents." }),
      category: s.enum(categories),
    },
    run: ({ tx, now }, args) => addItem(tx, args, new Date(now)),
  },
  removeItem: {
    description: "Remove a purchase from the receipt.",
    args: { item: s.string({ description: "The item's $id." }) },
    run: ({ tx }, { item }) => removeItem(tx, item),
  },
  setCurrency: {
    description: "Set the currency symbol on the receipt; up to 3 characters, blank becomes $.",
    args: { symbol: s.string() },
    run: ({ tx }, { symbol }) => setCurrency(tx, symbol),
  },
});
