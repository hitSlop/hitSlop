import { defineCommands } from "hitslop/commands";
import { refuse, s } from "hitslop";
import doc from "./schema";

export default defineCommands(doc, {
  addRow: {
    description: "Insert a row and return its identity.",
    args: { text: s.string() },
    run({ tx }, { text }) {
      if (!text.trim()) refuse("Text required.");
      return tx.fields.tasks.insert({ text });
    },
  },
  resetRow: {
    description: "Resolve a row argument and update its flags atomically.",
    args: { task: s.string({ description: "The task's $id." }) },
    run({ tx }, { task }) {
      tx.fields.tasks.item(task).done.set(false);
      tx.fields.tasks.item(task).archived.set(false);
    },
  },
  removeRow: {
    description: "Remove the resolved row.",
    args: { task: s.string({ description: "The task's $id." }) },
    run({ tx }, { task }) { tx.fields.tasks.remove(task); },
  },
});
