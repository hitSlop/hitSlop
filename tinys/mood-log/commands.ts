import { defineCommands } from "@hitslop/tinys/commands";
import { s } from "@hitslop/tinys";
import schema from "./schema";
import { addEntry, removeEntry, setPrompt } from "./entries";

export default defineCommands(schema, {
  addEntry: {
    description: "Log a check-in stamped with the current time and return its stable row ID.",
    args: {
      mood: s.integer({ min: 1, max: 5, description: "Mood from 1 (heavy) to 5 (radiant)." }),
      energy: s.integer({ min: 1, max: 5, description: "Energy from 1 to 5." }),
      note: s.string({ description: "One line about the day." }),
    },
    run: ({ tx, current, now }, args) => addEntry(tx, current, args, new Date(now)),
  },
  removeEntry: {
    description: "Remove a check-in from the journal by its $id.",
    args: { entry: s.string({ description: "The entry's $id." }) },
    run: ({ tx }, { entry }) => removeEntry(tx, entry),
  },
  setPrompt: {
    description: "Set the prompt shown at the top of the journal.",
    args: { prompt: s.string({ description: "The prompt text." }) },
    run: ({ tx }, { prompt }) => setPrompt(tx, prompt),
  },
});
