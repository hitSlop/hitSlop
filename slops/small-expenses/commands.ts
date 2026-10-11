import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc from "./schema";
import { addItem, clearNote, removeItem, settleItem } from "./items";

export default defineCommands(doc, {
  addItem: {
    description: "Add an unsettled expense and return its stable row ID.",
    args: {
      merchant: s.string({ minLength: 1 }),
      amountMinor: s.integer({ min: 0 }),
      note: s.optional(s.string()),
    },
    run: ({ tx }, args) => addItem(tx, args),
  },
  settleItem: {
    description: "Mark one expense as settled.",
    args: { item: s.string({ description: "The expense's $id." }) },
    run: ({ tx }, { item }) => settleItem(tx, item),
  },
  clearNote: {
    description: "Remove the note from one expense.",
    args: { item: s.string({ description: "The expense's $id." }) },
    run: ({ tx }, { item }) => clearNote(tx, item),
  },
  removeItem: {
    description: "Remove one expense from the list.",
    args: { item: s.string({ description: "The expense's $id." }) },
    run: ({ tx }, { item }) => removeItem(tx, item),
  },
});
