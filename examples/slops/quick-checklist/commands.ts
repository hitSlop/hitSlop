import { s } from "hitslop";
import doc from "./schema";

export const addTask = doc.command({
  description: "Add an unfinished task and return its stable row ID.",
  args: { text: s.string() },
  run({ tx }, { text }) {
    if (!text.trim()) throw new Error("Enter a task.");
    return tx.fields.tasks.insert({ text: text.trim(), done: false, archived: false });
  },
});
export const archiveFinished = doc.command({
  description: "File every completed task and return the number filed.",
  args: {},
  run({ current, tx }) {
    const tasks = current.tasks.filter(task => task.done && !task.archived);
    for (const task of tasks) tx.at(task).archived.set(true);
    return tasks.length;
  },
});
export const restoreTask = doc.command({
  description: "Return a filed task to the active list, unfinished.",
  args: { id: s.string({ minLength: 1 }) },
  run({ current, tx }, { id }) {
    const task = current.tasks.find(task => task.$id === id);
    if (!task) throw new Error("That task no longer exists.");
    tx.at(task).archived.set(false);
    tx.at(task).done.set(false);
  },
});

export const removeTask = doc.command({
  description: "Remove a task from the checklist.",
  args: { id: s.string({ minLength: 1 }) },
  run({ tx }, { id }) {
    tx.fields.tasks.remove(id);
  },
});
