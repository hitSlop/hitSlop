import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc, { themes } from "./schema";
import { addItem, removeItem, setDate, setTheme } from "./notes";

export default defineCommands(doc, {
  addItem: {
    description: "Add a Cornell note block with a cue and notes and return its stable row ID.",
    args: { cue: s.string(), notes: s.string() },
    run: ({ tx }, { cue, notes }) => addItem(tx, cue, notes),
  },
  removeItem: {
    description: "Remove a note block from the page.",
    args: { item: s.string({ description: "The note block's $id." }) },
    run: ({ tx }, { item }) => removeItem(tx, item),
  },
  setTheme: {
    description: "Switch the notebook page to light or dark.",
    args: { theme: s.enum(themes) },
    run: ({ tx }, { theme }) => setTheme(tx, theme),
  },
  setDate: {
    description: "Set the lecture date to a YYYY-MM-DD string.",
    args: { date: s.string({ description: "The lecture date, e.g. 2026-09-02." }) },
    run: ({ tx }, { date }) => setDate(tx, date),
  },
});
