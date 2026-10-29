import { refuse, type RowRef, type Tx, type Value } from "@hitslop/tinys";
import type schema from "./schema";
export type Plan = Value<typeof schema.descriptor>;
type Edit = Tx<typeof schema>;

const MAX_MINUTES = 180;

/** The editor and its export derive hours, counts and hour blocks from the same snapshot. */
export function planView(data: Plan) {
  const minutes = data.deepWork.minutes;
  const hours = Math.floor(minutes / 60);
  const mins = minutes % 60;
  const shortDone = data.shortTasks.filter(task => task.done).length;
  const maintDone = data.maintenance.filter(task => task.done).length;
  return {
    hours, mins,
    clock: `${hours}h ${String(mins).padStart(2, "0")}m`,
    shortDone, maintDone,
    totalDone: shortDone + maintDone,
    hourBlocks: [1, 2, 3].map(hour => ({
      hour,
      filled: minutes >= hour * 60,
      partial: minutes < hour * 60 && minutes > (hour - 1) * 60,
    })),
  };
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function adjustFocus(tx: Edit, current: Plan, delta: number) {
  if (!Number.isInteger(delta)) refuse("Focus change must be whole minutes.");
  const next = Math.max(0, Math.min(MAX_MINUTES, current.deepWork.minutes + delta));
  tx.fields.deepWork.minutes.set(next);
  return next;
}

/** Clicking the hour block that is already full steps back one hour, as the page always did. */
export function toggleHour(tx: Edit, current: Plan, hour: number) {
  if (!Number.isInteger(hour) || hour < 1 || hour > 3) refuse("Hour must be 1, 2 or 3.");
  const target = hour * 60;
  const next = current.deepWork.minutes === target ? (hour - 1) * 60 : target;
  tx.fields.deepWork.minutes.set(next);
  return next;
}

function findTask(data: Plan, task: RowRef) {
  const id = typeof task === "string" ? task : task.$id;
  const row = [...data.shortTasks, ...data.maintenance].find(item => item.$id === id);
  if (!row) refuse("No task has that id.");
  return row;
}

export function setTaskDone(tx: Edit, current: Plan, task: RowRef, done: boolean) {
  tx.at(findTask(current, task)).done.set(done);
}

export function renameTask(tx: Edit, current: Plan, task: RowRef, text: string) {
  tx.at(findTask(current, task)).text.set(text);
}

export function setDeepWork(tx: Edit, fields: { project?: string; notes?: string }) {
  if (fields.project === undefined && fields.notes === undefined) refuse("Give a project or notes to set.");
  if (fields.project !== undefined) tx.fields.deepWork.project.set(fields.project);
  if (fields.notes !== undefined) tx.fields.deepWork.notes.set(fields.notes);
}
