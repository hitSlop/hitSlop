import { refuse, type Tx, type Value } from "hitslop";
import type schema from "./schema";
export type Data = Value<typeof schema.descriptor>;
type Edit = Tx<typeof schema>;

/** The ledger has six slots. */
export const SLOTS = 6;

/** The editor and its export derive the count, percentage and focus slot from the same snapshot. */
export function docketView(data: Data) {
  const completed = data.tasks.filter(task => task.done).length;
  return {
    completed,
    percent: Math.round((completed / SLOTS) * 100),
    activeIndex: data.tasks.findIndex(task => !task.done),
  };
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function setDone(tx: Edit, task: string, done: boolean) {
  tx.fields.tasks.item(task).done.set(done);
}
export function setTaskText(tx: Edit, task: string, text: string) {
  tx.fields.tasks.item(task).text.set(text);
}
export function moveTask(tx: Edit, current: Data, task: string, direction: "up" | "down") {
  const index = current.tasks.findIndex(row => row.$id === task);
  if (index < 0) refuse("That task is not on the docket.");
  const neighbor = current.tasks[direction === "up" ? index - 1 : index + 1];
  if (!neighbor) refuse(direction === "up" ? "That task is already first." : "That task is already last.");
  if (direction === "up") tx.fields.tasks.move(task, { before: neighbor.$id });
  else tx.fields.tasks.move(task, { after: neighbor.$id });
}
/** Carries each unfinished task's text into the earliest slots, clears every done mark and sets the date. */
export function rollover(tx: Edit, current: Data, date: string) {
  const incomplete = current.tasks.filter(task => !task.done);
  for (let i = 0; i < current.tasks.length; i++) {
    const row = current.tasks[i];
    if (!row) continue;
    tx.at(row).text.set(incomplete[i]?.text ?? "");
    tx.at(row).done.set(false);
  }
  tx.fields.date.set(date);
  return incomplete.length;
}
export function setDate(tx: Edit, date: string) {
  tx.fields.date.set(date);
}
export function setNotes(tx: Edit, notes: string) {
  tx.fields.notes.set(notes);
}
