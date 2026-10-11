import { refuse, type Tx } from "hitslop";
import type doc from "./schema";
import { colors } from "./schema";

type Edit = Tx<typeof doc>;
export type Data = typeof doc.current;
export type Day = Data["days"][number];
export type Task = Day["tasks"][number];
export type TaskColor = (typeof colors)[number];

export const SNAP = 15;
export const HOUR = 64;
export const COLORS = colors;

export function minutes(time: string): number | null {
  if (!/^(?:[01]\d|2[0-3]):[0-5]\d$/.test(time)) return null;
  const [h, m] = time.split(":").map(Number);
  return h * 60 + m;
}

export function clock(value: number): string {
  return `${String(Math.floor(value / 60)).padStart(2, "0")}:${String(value % 60).padStart(2, "0")}`;
}

export function label(value: number): string {
  if (value === 1440) return "Midnight";
  return `${Math.floor(value / 60) % 12 || 12}${value % 60 ? ":" + String(value % 60).padStart(2, "0") : ""}${value < 720 ? "am" : "pm"}`;
}

export function duration(task: { durationMinutes?: number }): number {
  return Math.max(SNAP, Math.min(1440, task.durationMinutes ?? 60));
}

export function snap(value: number): number {
  return Math.round(value / SNAP) * SNAP;
}

export function move(start: number, span: number): number {
  return Math.max(0, Math.min(1440 - span, snap(start)));
}

export function resize(start: number, end: number, at: number, edge: "start" | "end") {
  return edge === "start"
    ? { start: Math.max(0, Math.min(end - SNAP, snap(at))), end }
    : { start, end: Math.min(1440, Math.max(start + SNAP, snap(at))) };
}

export function tasksByTime<T extends { time: string }>(tasks: readonly T[]): T[] {
  return [...tasks].sort((left, right) => {
    if (!left.time && right.time) return 1;
    if (left.time && !right.time) return -1;
    return left.time.localeCompare(right.time);
  });
}

export function layout(tasks: readonly Task[]) {
  const items = tasks
    .flatMap((task) => {
      const start = minutes(task.time);
      return start === null
        ? []
        : [{ task, start, end: Math.min(1440, start + duration(task)), column: 0, columns: 1 }];
    })
    .sort((a, b) => a.start - b.start || a.end - b.end);
  let cluster: typeof items = [];
  let boundary = 0;
  function flush() {
    const ends: number[] = [];
    for (const item of cluster) {
      let seat = ends.findIndex((end) => end <= item.start);
      if (seat < 0) seat = ends.length;
      ends[seat] = item.end;
      item.column = seat;
    }
    for (const item of cluster) item.columns = ends.length;
    cluster = [];
  }
  for (const item of items) {
    if (item.start >= boundary) flush();
    cluster.push(item);
    boundary = Math.max(boundary, item.end);
  }
  flush();
  return items;
}

/** The board and its export derive totals from the same snapshot. */
export function plannerView(data: Data) {
  const tasks = data.days.flatMap(day => day.tasks);
  return { tasks, total: tasks.length, done: tasks.filter(task => task.done).length };
}

export type BlockInput = {
  day: string;
  title: string;
  /** HH:MM, or empty for an anytime block. */
  time: string;
  durationMinutes?: number;
  color: TaskColor;
  done?: boolean;
};

function findDay(data: Data, name: string): Day {
  return data.days.find(day => day.name === name) ?? refuse(`There is no day named ${name}.`);
}
function findTask(data: Data, id: string) {
  for (const day of data.days) {
    const task = day.tasks.find(item => item.$id === id);
    if (task) return { day, task };
  }
  return refuse("There is no block with that $id.");
}

function checked(input: BlockInput) {
  const title = input.title.trim();
  if (!title) refuse("Give this block a title.");
  if (!colors.includes(input.color)) refuse(`Color must be one of ${colors.join(", ")}.`);
  const durationMinutes = input.durationMinutes;
  if (durationMinutes !== undefined && (!Number.isInteger(durationMinutes) || durationMinutes < 15 || durationMinutes > 1440)) {
    refuse("Duration must be a whole number of minutes from 15 to 1440.");
  }
  if (input.time !== "") {
    const start = minutes(input.time);
    if (start === null) refuse("Start time must be HH:MM on a 24-hour clock, or empty for an anytime block.");
    if (durationMinutes === undefined) refuse("A scheduled block needs a duration.");
    if (start + durationMinutes > 1440) refuse("A block must end by midnight.");
  }
  return { title, time: input.time, color: input.color, done: input.done ?? false, durationMinutes };
}

function rowFields(fields: ReturnType<typeof checked>) {
  const { durationMinutes, ...rest } = fields;
  return durationMinutes === undefined ? rest : { ...rest, durationMinutes };
}

export function addBlock(tx: Edit, current: Data, input: BlockInput) {
  const target = findDay(current, input.day);
  return tx.at(target).tasks.insert(rowFields(checked(input)));
}

/** Rewrite a block; a different day moves it, which gives it a new $id. */
export function updateBlock(tx: Edit, current: Data, id: string, input: BlockInput) {
  const { day: source, task } = findTask(current, id);
  const target = findDay(current, input.day);
  const fields = checked(input);
  if (source.$id !== target.$id) {
    tx.at(source).tasks.remove(task.$id);
    return tx.at(target).tasks.insert(rowFields(fields));
  }
  const handle = tx.at(task);
  handle.title.set(fields.title);
  handle.time.set(fields.time);
  handle.done.set(fields.done);
  handle.color.set(fields.color);
  if (fields.durationMinutes !== undefined) handle.durationMinutes.set(fields.durationMinutes);
  else if (task.durationMinutes !== undefined) handle.durationMinutes.clear();
  return { id: task.$id };
}

/** Place a block on a day at a time ("" = anytime), optionally resizing it. Returns its $id. */
export function moveBlock(tx: Edit, current: Data, id: string, dayName: string, time: string, durationMinutes?: number) {
  const { day: source, task } = findTask(current, id);
  const target = findDay(current, dayName);
  const span = durationMinutes ?? task.durationMinutes;
  if (time !== "") {
    const start = minutes(time);
    if (start === null) refuse("Start time must be HH:MM on a 24-hour clock, or empty for an anytime block.");
    if (span !== undefined && start + span > 1440) refuse("A block must end by midnight.");
  }
  if (durationMinutes !== undefined && (!Number.isInteger(durationMinutes) || durationMinutes < 15 || durationMinutes > 1440)) {
    refuse("Duration must be a whole number of minutes from 15 to 1440.");
  }
  if (source.$id === target.$id) {
    const handle = tx.at(task);
    handle.time.set(time);
    if (durationMinutes !== undefined) handle.durationMinutes.set(durationMinutes);
    return { id: task.$id };
  }
  tx.at(source).tasks.remove(task.$id);
  return tx.at(target).tasks.insert({
    time, title: task.title, done: task.done, color: task.color,
    ...(span === undefined ? {} : { durationMinutes: span }),
  });
}

export function setBlockDone(tx: Edit, current: Data, id: string, done: boolean) {
  tx.at(findTask(current, id).task).done.set(done);
}

export function removeBlock(tx: Edit, current: Data, id: string) {
  const { day, task } = findTask(current, id);
  tx.at(day).tasks.remove(task.$id);
}

/** Keyboard step: slide a block, or one of its edges, by `step` minutes. Returns the new start. */
export function nudgeBlock(tx: Edit, task: Task, step: number, edge?: "start" | "end") {
  const start = minutes(task.time);
  if (start === null) return refuse("Anytime blocks have no time to move.");
  const span = Math.min(duration(task), 1440 - start);
  const handle = tx.at(task);
  if (edge) {
    const next = resize(start, start + span, (edge === "start" ? start : start + span) + step, edge);
    handle.time.set(clock(next.start));
    handle.durationMinutes.set(next.end - next.start);
    return next.start;
  }
  const next = move(start + step, span);
  handle.time.set(clock(next));
  return next;
}

/** Keyboard step: send a block to the neighbouring day, keeping its time. Returns the new $id. */
export function shiftDay(tx: Edit, current: Data, id: string, direction: -1 | 1) {
  const { day, task } = findTask(current, id);
  const target = current.days[current.days.findIndex(item => item.$id === day.$id) + direction];
  if (!target) return refuse("There is no day in that direction.");
  return moveBlock(tx, current, id, target.name, task.time);
}
