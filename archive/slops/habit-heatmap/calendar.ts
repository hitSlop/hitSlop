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
export function streak(checkins: Readonly<Record<string, number>>, today: string): number {
  let day = checkins[today] ? today : shiftDay(today, -1);
  let count = 0;
  while (checkins[day] > 0) { count++; day = shiftDay(day, -1); }
  return count;
}
export function completedDays(checkins: Readonly<Record<string, number>>, days: readonly string[], today: string): number {
  return days.filter(day => day <= today && checkins[day] > 0).length;
}
export const labelDay = (day: string) => new Intl.DateTimeFormat(undefined, {
  weekday: "long", month: "long", day: "numeric", year: "numeric",
}).format(localDate(day));
export const shortDay = (day: string) => new Intl.DateTimeFormat(undefined, {
  month: "short", day: "numeric",
}).format(localDate(day));
