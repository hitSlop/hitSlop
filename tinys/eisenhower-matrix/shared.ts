import { refuse, type Tx, type Value } from "@hitslop/tinys";
import { zones } from "./schema";
import type schema from "./schema";

export type Zone = (typeof zones)[number];
export type QuadrantKey = Exclude<Zone, "inbox">;
export type Data = Value<typeof schema.descriptor>;
export type Task = Data["tasks"][number];
type Edit = Tx<typeof schema>;

export const QUADRANTS: { key: QuadrantKey; num: string; title: string; subtitle: string; tag: string; empty: string }[] = [
  { key: "q1", num: "I", title: "Do First", subtitle: "Urgent & Important", tag: "Crises & Deadlines", empty: "No fires on the blotter." },
  { key: "q2", num: "II", title: "Schedule", subtitle: "Not Urgent & Important", tag: "Focus & Leverage", empty: "Nothing to grow yet." },
  { key: "q3", num: "III", title: "Delegate", subtitle: "Urgent & Not Important", tag: "Interruptions", empty: "Nothing to hand off." },
  { key: "q4", num: "IV", title: "Don’t Do", subtitle: "Not Urgent & Not Important", tag: "Eliminate", empty: "Nothing to drop." },
];

/** The page and its export derive lists, counts and the leverage ratio from one snapshot. */
export function matrixView(data: Data) {
  const inZone = (zone: Zone): Task[] => data.tasks.filter(task => task.zone === zone);
  const open = data.tasks.filter(task => !task.done);
  const q2Open = open.filter(task => task.zone === "q2").length;
  return {
    inZone,
    inbox: inZone("inbox"),
    open: open.length,
    q2Ratio: open.length > 0 ? Math.round((q2Open / open.length) * 100) : 0,
    filled: Object.fromEntries(QUADRANTS.map(q => [q.key, open.some(task => task.zone === q.key)])) as Record<QuadrantKey, boolean>,
  };
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
/** Places a task in the holding pen (or another square). Returns the new row's $id. */
export function addTask(tx: Edit, text: string, zone: Zone = "inbox") {
  const clean = text.trim();
  if (!clean) refuse("Enter a task.");
  return tx.fields.tasks.insert({ text: clean, zone });
}
/** Stamps a task into square I-IV. The holding pen is not a stamp target. Returns false if it is already there. */
export function moveTask(tx: Edit, current: Data, id: string, to: Zone) {
  if (to === "inbox") refuse("Stamp a task into square I–IV.");
  const task = current.tasks.find(row => row.$id === id);
  if (!task) refuse("No such task.");
  if (task.zone === to) return false;
  tx.fields.tasks.item(id).zone.set(to);
  return true;
}
export function setDone(tx: Edit, id: string, done: boolean) {
  tx.fields.tasks.item(id).done.set(done);
}
export function removeTask(tx: Edit, id: string) {
  tx.fields.tasks.remove(id);
}
