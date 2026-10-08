import { refuse, s } from "hitslop";
import doc from "./schema";

export const addRow = doc.command({
  description: "Insert a row and return its identity.",
  args: { text: s.string() },
  run({ tx }, { text }) {
    if (!text.trim()) refuse("Text required.");
    return tx.fields.tasks.insert({ text });
  },
});
export const resetRow = doc.command({
  description: "Resolve a row argument and update its flags atomically.",
  args: { task: s.row("tasks") },
  run({ tx }, { task }) {
    tx.at(task).done.set(false);
    tx.at(task).archived.set(false);
  },
});
export const removeRow = doc.command({
  description: "Remove the resolved row.",
  args: { task: s.row("tasks") },
  run({ tx }, { task }) { tx.fields.tasks.remove(task.$id); },
});
