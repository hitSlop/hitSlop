import type { Kind } from "./schema";

export const DAY_START = 6 * 60;
export const DAY_END = 24 * 60;
export const SNAP = 15;
export const HOURS = Array.from(
  { length: (DAY_END - DAY_START) / 60 },
  (_, index) => DAY_START + index * 60,
);

type Timed = { start: string; end: string };

export function localDate(value: Date): string {
  return `${value.getFullYear()}-${String(value.getMonth() + 1).padStart(2, "0")}-${String(value.getDate()).padStart(2, "0")}`;
}

export function today(): string {
  return localDate(new Date());
}

export function formatDate(value: string, options: Intl.DateTimeFormatOptions): string {
  if (!validDate(value)) return "—";
  const parsed = new Date(`${value}T00:00:00`);
  return Number.isNaN(parsed.valueOf()) ? "—" : parsed.toLocaleDateString(undefined, options);
}

export function toMinutes(value: string): number {
  if (!/^([01]\d|2[0-3]):[0-5]\d$/.test(value)) return DAY_START;
  const [hours, minutes] = value.split(":").map(Number);
  return hours * 60 + minutes;
}

export function toClock(total: number): string {
  const wrapped = ((total % 1440) + 1440) % 1440;
  return `${String(Math.floor(wrapped / 60)).padStart(2, "0")}:${String(wrapped % 60).padStart(2, "0")}`;
}

export function toLabel(total: number): string {
  const hour = Math.floor(total / 60) % 24;
  const suffix = hour < 12 ? "am" : "pm";
  const shown = hour % 12 === 0 ? 12 : hour % 12;
  return total % 60 === 0 ? `${shown} ${suffix}` : `${shown}:${String(total % 60).padStart(2, "0")} ${suffix}`;
}

export function clampDay(total: number): number {
  return Math.min(Math.max(total, DAY_START), DAY_END);
}

export function offsetOf(total: number): number {
  return (clampDay(total) - DAY_START) / 60;
}

export function rangeOf(block: Timed): { start: number; end: number } {
  const start = Math.min(clampDay(toMinutes(block.start)), DAY_END - SNAP);
  const raw = toMinutes(block.end);
  const end = raw === 0 ? DAY_END : raw;
  return { start, end: Math.min(Math.max(end, start + SNAP), DAY_END) };
}

export type Placed<T> = {
  block: T;
  start: number;
  end: number;
  column: number;
  columns: number;
};

export function layout<T extends Timed>(blocks: readonly T[]): Placed<T>[] {
  const items = blocks
    .map((block) => ({ block, ...rangeOf(block) }))
    .sort((a, b) => a.start - b.start || a.end - b.end);
  const output: Placed<T>[] = [];
  let cluster: typeof items = [];
  let clusterEnd = Number.NEGATIVE_INFINITY;
  const flush = (): void => {
    const columnEnds: number[] = [];
    const seats = cluster.map((item) => {
      let column = columnEnds.findIndex((end) => end <= item.start);
      if (column === -1) column = columnEnds.length;
      columnEnds[column] = item.end;
      return { item, column };
    });
    for (const { item, column } of seats) output.push({ ...item, column, columns: columnEnds.length });
    cluster = [];
    clusterEnd = Number.NEGATIVE_INFINITY;
  };
  for (const item of items) {
    if (cluster.length > 0 && item.start >= clusterEnd) flush();
    cluster.push(item);
    clusterEnd = Math.max(clusterEnd, item.end);
  }
  if (cluster.length > 0) flush();
  return output;
}

export function bookedMinutesOf(blocks: readonly Timed[]): number {
  let total = 0;
  let covered = DAY_START;
  for (const item of layout(blocks)) {
    total += Math.max(0, item.end - Math.max(covered, item.start));
    covered = Math.max(covered, item.end);
  }
  return total;
}

export function isKind(value: string): value is Kind {
  return value === "focus" || value === "meeting" || value === "break" || value === "personal";
}

export function validDate(value: string): boolean {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) return false;
  const date = new Date(`${value}T12:00:00`);
  return !Number.isNaN(date.valueOf()) && localDate(date) === value;
}

export function validRange(start: string, end: string): boolean {
  const valid = (value: string) => /^([01]\d|2[0-3]):[0-5]\d$/.test(value);
  if (!valid(start) || !valid(end)) return false;
  const a = toMinutes(start);
  const b = end === "00:00" ? DAY_END : toMinutes(end);
  return a >= DAY_START && b <= DAY_END && b - a >= SNAP;
}

export function adjustRange(
  range: { start: number; end: number },
  mode: "move" | "start" | "end",
  delta: number,
): { start: number; end: number } {
  const { start, end } = range;
  delta = Math.round(delta / SNAP) * SNAP;
  if (mode === "move") {
    const next = Math.min(Math.max(start + delta, DAY_START), DAY_END - (end - start));
    return { start: next, end: next + (end - start) };
  }
  if (mode === "start") return { start: Math.min(Math.max(start + delta, DAY_START), end - SNAP), end };
  return { start, end: Math.max(Math.min(end + delta, DAY_END), start + SNAP) };
}

export function scrollForTime(
  total: number,
  hourHeight: number,
  viewportHeight: number,
  contentHeight: number,
): number {
  return Math.max(
    0,
    Math.min(
      ((clampDay(total) - DAY_START) / 60) * hourHeight + 10 - viewportHeight / 3,
      Math.max(0, contentHeight - viewportHeight),
    ),
  );
}
