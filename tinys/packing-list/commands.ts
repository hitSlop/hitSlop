import { defineCommands } from "@hitslop/tinys/commands";
import { s } from "@hitslop/tinys";
import schema from "./schema";
import { addItem, applyPreset, clearPacked, removeItem, setPacked } from "./items";

export default defineCommands(schema, {
  addItem: {
    description: "Add an unpacked item to a bag category and return its stable row ID.",
    args: {
      text: s.string({ description: "What to pack." }),
      category: s.string({ description: "The bag category's key, such as carry or tech." }),
      quantity: s.integer({ min: 1, max: 9, description: "How many, from 1 to 9." }),
    },
    run: ({ tx, current }, args) => addItem(tx, current, args),
  },
  setPacked: {
    description: "Mark an item packed or unpacked by its $id.",
    args: {
      item: s.string({ description: "The item's $id." }),
      packed: s.boolean({ description: "True when the item is in the bag." }),
    },
    run: ({ tx }, { item, packed }) => setPacked(tx, item, packed),
  },
  removeItem: {
    description: "Remove an item from the tag by its $id.",
    args: { item: s.string({ description: "The item's $id." }) },
    run: ({ tx }, { item }) => removeItem(tx, item),
  },
  clearPacked: {
    description: "Remove every packed item and return the number removed.",
    args: {},
    run: ({ tx, current }) => clearPacked(tx, current),
  },
  applyPreset: {
    description: "Add a travel preset's items that the tag does not already list and return the number added.",
    args: {
      preset: s.string({ description: "The preset's id: weekend, international, outdoor or business." }),
    },
    run: ({ tx, current }, { preset }) => applyPreset(tx, current, preset),
  },
});
