import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc from "./schema";
import { addCategory, logExpense, removeCategory, renameCategory, setAllocated } from "./budget";

const category = s.string({ description: "The category's $id." });

export default defineCommands(doc, {
  addCategory: {
    description: "Add a spending category and return its stable row ID.",
    args: { name: s.string(), allocated: s.number({ min: 0 }) },
    run: ({ tx }, { name, allocated }) => addCategory(tx, name, allocated),
  },
  renameCategory: {
    description: "Change a category's name.",
    args: { category, name: s.string() },
    run: ({ tx }, { category, name }) => renameCategory(tx, category, name),
  },
  setAllocated: {
    description: "Set a category's monthly allocation.",
    args: { category, amount: s.number({ min: 0 }) },
    run: ({ tx }, { category, amount }) => setAllocated(tx, category, amount),
  },
  logExpense: {
    description: "Add an amount to a category's spent total and return the category's name.",
    args: { category, amount: s.number({ min: 0 }) },
    run: ({ tx, current }, { category, amount }) => logExpense(tx, current, category, amount),
  },
  removeCategory: {
    description: "Remove a spending category from the budget.",
    args: { category },
    run: ({ tx }, { category }) => removeCategory(tx, category),
  },
});
