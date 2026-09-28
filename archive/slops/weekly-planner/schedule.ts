import type { Task } from "./schema";

export const SNAP = 15;
export const HOUR = 64;
export const COLORS = ["sky", "coral", "mint", "lilac"] as const;

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
