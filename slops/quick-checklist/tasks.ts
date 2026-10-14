import { refuse, type RowRef, type Tx, type Value } from "hitslop";
import type schema from "./schema";
export type Checklist = Value<typeof schema.descriptor>;
type Edit = Tx<typeof schema>;

/** The editor and its captures derive their lists, counts and ordering from the same snapshot. */
export function checklistView(data: Checklist) {
  const visible = data.tasks.filter(task => !task.archived);
  const filed = data.tasks.filter(task => task.archived);
  const finished = visible.filter(task => task.done).length;
  return {
    visible, filed, finished,
    ratio: visible.length ? finished / visible.length * 100 : 0,
    marks: visible.length ? Math.round(3 * finished / visible.length) : 0,
  };
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function addTask(tx: Edit, text: string) {
  if (!text.trim()) refuse("Enter a task.");
  return tx.fields.tasks.insert({ text: text.trim() });
}
export function fileFinished(tx: Edit, current: Checklist) {
  const tasks = current.tasks.filter(task => task.done && !task.archived);
  for (const task of tasks) tx.at(task).archived.set(true);
  return tasks.length;
}
export function restoreTask(tx: Edit, task: RowRef) {
  const row = tx.fields.tasks.item(task);
  row.archived.set(false);
  row.done.set(false);
}
