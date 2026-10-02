import { CalendarDate, getLocalTimeZone, parseDate, type DateValue } from "@internationalized/date";

export type View = "am" | "pm" | "all";

export function initialView(data: { morningDone: boolean; eveningDone: boolean }): View {
  return data.morningDone ? (data.eveningDone ? "all" : "pm") : "am";
}

export function calendarDate(value: string): CalendarDate | undefined {
  try {
    return parseDate(value);
  } catch {
    return undefined;
  }
}

export function displayDate(value: string): string {
  const date = calendarDate(value);
  return date
    ? new Intl.DateTimeFormat(undefined, { weekday: "long", month: "short", day: "numeric", year: "numeric" }).format(date.toDate(getLocalTimeZone()))
    : value.trim() || "Choose a day";
}

export function dateString(value: DateValue): string {
  return new CalendarDate(value.year, value.month, value.day).toString();
}
