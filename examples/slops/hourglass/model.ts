import type { Hourglass } from "./schema";

const minute = 60_000;
const hour = 60 * minute;
const day = 24 * hour;

/** Durations the glass can be turned over for. */
export const presets = [
  { label: "25 min", duration: 25 * minute },
  { label: "1 hour", duration: hour },
  { label: "1 day", duration: day },
  { label: "1 week", duration: 7 * day },
];

export type Reading =
  | { state: "unset"; remaining: 0 }
  | { state: "running"; remaining: number; value: string; unit: string; until: string }
  | { state: "done"; remaining: 0; until: string };

const pad = (n: number) => String(n).padStart(2, "0");
const sameDay = (a: Date, b: Date) => a.toDateString() === b.toDateString();
const time = new Intl.DateTimeFormat(undefined, { hour: "numeric", minute: "2-digit" });
const date = new Intl.DateTimeFormat(undefined, {
  weekday: "short", day: "numeric", month: "short", hour: "numeric", minute: "2-digit",
});
/** The end as a person would say it: a time today, otherwise a day and time. */
export function describeEnd(end: number, now: number): string {
  const at = new Date(end);
  return sameDay(at, new Date(now)) ? time.format(at) : date.format(at);
}

/** What the glass shows at `now`: how much sand is left (0 to 1) and the readout. Days for a
 * long wait; a clock under a day. */
export function read({ start, end }: Hourglass, now: number): Reading {
  if (!end || end <= start) return { state: "unset", remaining: 0 };
  const until = describeEnd(end, now);
  const left = end - now;
  if (left <= 0) return { state: "done", remaining: 0, until };
  const remaining = Math.min(left / (end - start), 1);
  if (left >= day) {
    const days = Math.floor(left / day);
    return { state: "running", remaining, value: String(days), unit: days === 1 ? "day to go" : "days to go", until };
  }
  const seconds = Math.ceil(left / 1000);
  const [h, m, s] = [Math.floor(seconds / 3600), Math.floor(seconds / 60) % 60, seconds % 60];
  const value = h ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`;
  return { state: "running", remaining, value, unit: "to go", until };
}

/** A `datetime-local` input's value for an epoch time, in local time. */
export function localInput(at: number): string {
  const d = new Date(at);
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}
