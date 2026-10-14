import { dateLabel } from "../_shared/format";
import { refuse, type RowRef, type Tx, type Value } from "hitslop";
import type schema from "./schema";
import type { colors } from "./schema";
/** Local calendar dates, deliberately independent of UTC offsets and 24-hour durations. */
export function dayKey(date: Date): string {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}
export function localDate(day: string): Date {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(day);
  if (!match) throw new RangeError("Expected a local calendar date");
  const date = new Date(0);
  date.setFullYear(Number(match[1]), Number(match[2]) - 1, Number(match[3]));
  date.setHours(12, 0, 0, 0);
  if (dayKey(date) !== day) throw new RangeError("Invalid calendar date");
  return date;
}
export function shiftDay(day: string, offset: number): string {
  const date = localDate(day);
  date.setDate(date.getDate() + offset);
  return dayKey(date);
}
/** Twelve Monday–Sunday columns, ending with the current calendar week. */
export function calendarDays(today: string): string[] {
  const weekday = (localDate(today).getDay() + 6) % 7;
  const start = shiftDay(today, -weekday - 11 * 7);
  return Array.from({ length: 84 }, (_, index) => shiftDay(start, index));
}
/** The calendar's days, its week columns and their labels: the month where one begins. */
export function calendarWeeks(today: string) {
  const days = calendarDays(today);
  const weeks = days.filter((_, i) => i % 7 === 0);
  const labels = weeks.map((day, index) =>
    index === 0 || day.slice(0, 7) !== weeks[index - 1]!.slice(0, 7)
      ? dateLabel(localDate(day), { month: "short" })
      : String(localDate(day).getDate()),
  );
  return { days, weeks, labels };
}
export const weekdays = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
export function streak(checkins: Readonly<Record<string, number>>, today: string): number {
  let day = checkins[today] ? today : shiftDay(today, -1);
  let count = 0;
  while (checkins[day] > 0) { count++; day = shiftDay(day, -1); }
  return count;
}
export function completedDays(checkins: Readonly<Record<string, number>>, days: readonly string[], today: string): number {
  return days.filter(day => day <= today && checkins[day] > 0).length;
}
export const labelDay = (day: string) => dateLabel(localDate(day), {
  weekday: "long", month: "long", day: "numeric", year: "numeric",
});
export const shortDay = (day: string) => dateLabel(localDate(day), {
  month: "short", day: "numeric",
});

// Edits are written once: the page runs them with doc.change, a command with its tx.
export type Edit = Tx<typeof schema>;
export type Data = Value<typeof schema.descriptor>;
export type HabitColor = (typeof colors)[number];

/** The editor and its export derive the calendar and per-habit numbers from the same snapshot. */
export function heatmapView(data: Data, today: string) {
  const { days, labels } = calendarWeeks(today);
  return {
    days, labels,
    habits: data.habits.map(habit => ({
      habit,
      streak: streak(habit.checkins, today),
      completed: completedDays(habit.checkins, days, today),
    })),
  };
}

function validDay(day: string): string {
  try { localDate(day); } catch { refuse("Use a calendar date written YYYY-MM-DD."); }
  return day;
}
export function addHabit(tx: Edit, name: string, color: HabitColor) {
  if (!name.trim()) refuse("Give this habit a name.");
  return tx.fields.habits.insert({ name: name.trim(), color, checkins: {} });
}
export function editHabit(tx: Edit, habit: RowRef, name: string, color: HabitColor) {
  if (!name.trim()) refuse("Give this habit a name.");
  const row = tx.fields.habits.item(habit);
  row.name.set(name.trim());
  row.color.set(color);
}
/** Mark or clear one day. Days after `today` cannot be checked in. */
export function setCheckin(tx: Edit, habit: RowRef, day: string, done: boolean, today: string) {
  validDay(day);
  if (day > today) refuse("That day has not happened yet.");
  const checkins = tx.fields.habits.item(habit).checkins;
  if (done) checkins.put(day, 1);
  else checkins.delete(day);
}
export function removeHabit(tx: Edit, habit: RowRef) {
  tx.fields.habits.remove(habit);
}
