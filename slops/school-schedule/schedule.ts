import { refuse, type Tx, type Value } from "hitslop";
import type schema from "./schema";
import { COLORS, DAYS } from "./schema";

export type Data = Value<typeof schema.descriptor>;
type Edit = Tx<typeof schema>;
export type Period = Data["periods"][number];
export type ClassEntry = Data["classes"][number];
export type Activity = Data["activities"][number];
export type DayKey = (typeof DAYS)[number];
export type SubjectColor = (typeof COLORS)[number];

export const DAY_NAMES: Record<DayKey, string> = {
  MON: "Monday",
  TUE: "Tuesday",
  WED: "Wednesday",
  THU: "Thursday",
  FRI: "Friday",
};

export const COLOR_NAMES: Record<SubjectColor, string> = {
  sage: "Mint",
  slate: "Sky",
  amber: "Peach",
  terracotta: "Coral",
  indigo: "Lilac",
  rose: "Pink",
  teal: "Aqua",
};

export function isDay(value: string): value is DayKey {
  return (DAYS as readonly string[]).includes(value);
}

export function isColor(value: string): value is SubjectColor {
  return (COLORS as readonly string[]).includes(value);
}

export function isBreakName(name: string): boolean {
  const label = name.toLowerCase();
  return label.includes("lunch") || label.includes("advisory") || label.includes("break") || label.includes("recess");
}

export function minutes(value: string): number | null {
  if (!/^([01]\d|2[0-3]):[0-5]\d$/.test(value)) return null;
  const [h, m] = value.split(":").map(Number);
  return h * 60 + m;
}

export function validTimes(start: string, end: string): boolean {
  const a = minutes(start);
  const b = minutes(end);
  return a !== null && b !== null && b > a;
}

export function clockLabel(value: string): string {
  const n = minutes(value);
  if (n === null) return "—";
  return `${Math.floor(n / 60) % 12 || 12}:${String(n % 60).padStart(2, "0")}${n < 720 ? "am" : "pm"}`;
}

export function sortedPeriods(periods: readonly Period[]): Period[] {
  return [...periods].sort((a, b) => (minutes(a.start) ?? 1440) - (minutes(b.start) ?? 1440));
}

/** The class in one timetable cell. */
export function classAt(data: Data, day: string, periodKey: string): ClassEntry | undefined {
  return data.classes.find((c) => c.day === day && c.periodKey === periodKey);
}

/** One day's after-class plans, earliest first. */
export function activitiesOn(data: Data, day: string): Activity[] {
  return data.activities.filter((a) => a.day === day).sort((a, b) => a.start.localeCompare(b.start));
}

/** The editor and its export capture derive the timetable from the same snapshot. */
export function scheduleView(data: Data) {
  return { periods: sortedPeriods(data.periods) };
}

/** Bell times for a new period: it starts when the last one ends and runs 50 minutes. */
export function nextPeriodDraft(periods: readonly Period[]) {
  const last = sortedPeriods(periods).at(-1);
  const start = last?.end || "08:00";
  const [h, m] = start.split(":").map(Number);
  const end = Math.min(h * 60 + m + 50, 1439);
  return {
    name: `Period ${periods.length + 1}`,
    start,
    end: `${String(Math.floor(end / 60)).padStart(2, "0")}:${String(end % 60).padStart(2, "0")}`,
  };
}

/** A period key from a random draw in [0, 1), so callers pass the random source. */
export function periodKeyFrom(draw: number): string {
  return `period-${Math.floor(draw * 2 ** 32).toString(36)}`;
}

export function liveStatus(data: Data, now: Date) {
  const day = now.getDay() > 0 && now.getDay() < 6 ? DAYS[now.getDay() - 1] : null;
  if (!day)
    return { day: null, periodKey: null, label: "Weekend", title: "A little time off.", detail: "Your week picks up on Monday.", next: "" };
  const at = now.getHours() * 60 + now.getMinutes();
  const periods = sortedPeriods(data.periods).filter((p) => validTimes(p.start, p.end));
  const events = [
    ...periods.map((p) => {
      const c = data.classes.find((item) => item.day === day && item.periodKey === p.periodKey);
      return {
        start: minutes(p.start)!,
        end: minutes(p.end)!,
        time: p.start,
        title: c?.subject.trim() || (isBreakName(p.name) ? p.name : "Free period"),
        room: c?.room || "",
        periodKey: p.periodKey as string | null,
        kind: c?.subject.trim() ? "In class" : isBreakName(p.name) ? p.name : "Free period",
      };
    }),
    ...data.activities
      .filter((a) => a.day === day && validTimes(a.start, a.end))
      .map((a) => ({
        start: minutes(a.start)!,
        end: minutes(a.end)!,
        time: a.start,
        title: a.title || "After-class plans",
        room: a.location,
        periodKey: null as string | null,
        kind: "After class",
      })),
  ].sort((a, b) => a.start - b.start);
  const current = events.find((e) => e.start <= at && at < e.end);
  const next = events.find((e) => e.start > at);
  if (current)
    return {
      day,
      periodKey: current.periodKey,
      label: current.kind,
      title: current.title,
      detail: [current.room, `${current.end - at} min left`].filter(Boolean).join(" · "),
      next: next ? `Next: ${next.title}${next.room ? ` · ${next.room}` : ""} · ${clockLabel(next.time)}` : "That’s your last scheduled stop.",
    };
  if (next)
    return {
      day,
      periodKey: null,
      label: events.some((e) => e.end <= at) ? "Up next" : "Before school",
      title: next.title,
      detail: [next.room, `in ${next.start - at} min`].filter(Boolean).join(" · "),
      next: `Starts at ${clockLabel(next.time)}`,
    };
  return {
    day,
    periodKey: null,
    label: events.length ? "All done" : "Open day",
    title: events.length ? "That’s a wrap." : "Room for something good.",
    detail: events.length ? "Nothing else on the schedule today." : "Add a class or an after-class plan.",
    next: "",
  };
}

// Validation returns the message the dialog shows; the edits refuse with the same text.

export type ClassInput = { day: string; periodKey: string; subject: string; room: string; teacher: string; color: string };

/** `id` is the class being edited; leave it out when adding. */
export function classError(data: Data, input: ClassInput, id?: string): string {
  if (!input.subject.trim()) return "Give this class a name.";
  if (!isDay(input.day) || !isColor(input.color) || !data.periods.some((p) => p.periodKey === input.periodKey))
    return "Choose a day and period.";
  if (id === undefined && classAt(data, input.day, input.periodKey)) return "That slot is occupied. Choose an empty slot.";
  if (id !== undefined && !data.classes.some((c) => c.$id === id)) return "That class no longer exists.";
  return "";
}

export function periodError(period: { periodKey: string; name: string; start: string; end: string }, periods: readonly Period[]): string {
  if (!period.name.trim()) return "Give this period a name.";
  if (!validTimes(period.start, period.end)) return "Choose an end time after the start time.";
  const a = minutes(period.start)!;
  const b = minutes(period.end)!;
  if (periods.some((p) => p.periodKey !== period.periodKey && validTimes(p.start, p.end) && a < minutes(p.end)! && b > minutes(p.start)!))
    return "This overlaps another period. Adjust the bell times.";
  return "";
}

export type ActivityInput = { day: string; start: string; end: string; title: string; location: string };

export function activityError(input: ActivityInput): string {
  if (!input.title.trim()) return "Give your plan a title.";
  if (!isDay(input.day) || !validTimes(input.start, input.end)) return "Choose a day and an end time after the start time.";
  return "";
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.

/** Adds a class (no `id`) or saves one; saving into an occupied slot swaps the two. Returns the class's $id. */
export function saveClass(tx: Edit, data: Data, input: ClassInput, id?: string): string {
  const error = classError(data, input, id);
  if (error) refuse(error);
  const day = input.day as DayKey;
  const color = input.color as SubjectColor;
  const fields = { subject: input.subject.trim(), room: input.room, teacher: input.teacher, color };
  if (id === undefined) return tx.fields.classes.insert({ periodKey: input.periodKey, day, ...fields }).id;
  const row = data.classes.find((c) => c.$id === id)!;
  const other = data.classes.find((c) => c.$id !== id && c.day === day && c.periodKey === input.periodKey);
  if (other) {
    tx.at(other).day.set(row.day);
    tx.at(other).periodKey.set(row.periodKey);
  }
  const handle = tx.at(row);
  handle.day.set(day);
  handle.periodKey.set(input.periodKey);
  handle.subject.set(fields.subject);
  handle.room.set(fields.room);
  handle.teacher.set(fields.teacher);
  handle.color.set(color);
  return id;
}

/** Drops a class into a cell, swapping with whatever is there. */
export function moveClass(tx: Edit, data: Data, id: string, day: string, periodKey: string) {
  const source = data.classes.find((c) => c.$id === id);
  if (!source) refuse("That class no longer exists.");
  if (!isDay(day) || !data.periods.some((p) => p.periodKey === periodKey)) refuse("Choose a day and period.");
  const other = data.classes.find((c) => c.$id !== id && c.day === day && c.periodKey === periodKey);
  if (other) {
    tx.at(other).day.set(source.day);
    tx.at(other).periodKey.set(source.periodKey);
  }
  tx.at(source).day.set(day);
  tx.at(source).periodKey.set(periodKey);
}

export function removeClass(tx: Edit, data: Data, id: string) {
  if (!data.classes.some((c) => c.$id === id)) refuse("That class no longer exists.");
  tx.fields.classes.remove(id);
}

/** Adds a period (no `id`) or changes its name and bell times. Returns the period's $id. */
export function savePeriod(tx: Edit, data: Data, input: { periodKey: string; name: string; start: string; end: string }, id?: string): string {
  const error = periodError(input, data.periods);
  if (error) refuse(error);
  if (id === undefined) {
    if (data.periods.some((p) => p.periodKey === input.periodKey)) refuse("That period key is already in use.");
    return tx.fields.periods.insert({ periodKey: input.periodKey, name: input.name, start: input.start, end: input.end }).id;
  }
  const row = data.periods.find((p) => p.$id === id);
  if (!row) refuse("That period no longer exists.");
  tx.at(row).name.set(input.name);
  tx.at(row).start.set(input.start);
  tx.at(row).end.set(input.end);
  return id;
}

/** Removes a period and every class in it; returns how many classes went with it. */
export function removePeriod(tx: Edit, data: Data, id: string) {
  const row = data.periods.find((p) => p.$id === id);
  if (!row) refuse("That period no longer exists.");
  const doomed = data.classes.filter((c) => c.periodKey === row.periodKey);
  tx.fields.periods.remove(id);
  for (const c of doomed) tx.fields.classes.remove(c.$id);
  return doomed.length;
}

/** Adds an after-class plan (no `id`) or saves one. Returns the plan's $id. */
export function saveActivity(tx: Edit, data: Data, input: ActivityInput, id?: string): string {
  const error = activityError(input);
  if (error) refuse(error);
  const day = input.day as DayKey;
  if (id === undefined) return tx.fields.activities.insert({ day, start: input.start, end: input.end, title: input.title, location: input.location }).id;
  const row = data.activities.find((a) => a.$id === id);
  if (!row) refuse("That plan no longer exists.");
  const handle = tx.at(row);
  handle.day.set(day);
  handle.start.set(input.start);
  handle.end.set(input.end);
  handle.title.set(input.title);
  handle.location.set(input.location);
  return id;
}

export function removeActivity(tx: Edit, data: Data, id: string) {
  if (!data.activities.some((a) => a.$id === id)) refuse("That plan no longer exists.");
  tx.fields.activities.remove(id);
}

export function saveDetails(tx: Edit, details: { studentName: string; term: string; homeroom: string; locker: string }) {
  tx.fields.studentName.set(details.studentName);
  tx.fields.term.set(details.term);
  tx.fields.homeroom.set(details.homeroom);
  tx.fields.locker.set(details.locker);
}

/** Repeats one day's classes across the other four weekdays; returns the number of classes copied. */
export function copyDay(tx: Edit, data: Data, from: string) {
  if (!isDay(from)) refuse("Choose a day to copy.");
  const source = data.classes.filter((c) => c.day === from);
  for (const dest of DAYS) {
    if (dest === from) continue;
    const matched = new Set<string>();
    for (const row of data.classes.filter((c) => c.day === dest)) {
      const origin = source.find((c) => c.periodKey === row.periodKey);
      if (!origin) {
        tx.fields.classes.remove(row.$id);
        continue;
      }
      matched.add(origin.$id);
      const handle = tx.at(row);
      handle.subject.set(origin.subject);
      handle.room.set(origin.room);
      handle.teacher.set(origin.teacher);
      handle.color.set(origin.color);
    }
    for (const origin of source) {
      if (matched.has(origin.$id)) continue;
      tx.fields.classes.insert({
        periodKey: origin.periodKey,
        day: dest,
        subject: origin.subject,
        room: origin.room,
        teacher: origin.teacher,
        color: origin.color,
      });
    }
  }
  return source.length;
}
