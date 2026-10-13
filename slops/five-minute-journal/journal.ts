import { dateLabel } from "../_shared/format";
import { CalendarDate, parseDate, type DateValue } from "@internationalized/date";
import { refuse, type Tx } from "hitslop";
import type doc from "./schema";

export type Edit = Tx<typeof doc>;
export type Data = typeof doc.current;
export type View = "am" | "pm" | "all";
export type Period = "am" | "pm";

/** Every free-text line of the journal, in page order. */
export const lineKeys = [
  "gratitude1", "gratitude2", "gratitude3",
  "intention1", "intention2", "intention3",
  "affirmation",
  "highlight1", "highlight2", "highlight3",
  "lesson",
  "quote", "quoteAuthor",
] as const;
export type LineKey = (typeof lineKeys)[number];

export const prompts = {
  gratitudes: [
    { key: "gratitude1", placeholder: "A small comfort…" },
    { key: "gratitude2", placeholder: "Someone who makes a difference…" },
    { key: "gratitude3", placeholder: "Something you might otherwise miss…" },
  ],
  intentions: [
    { key: "intention1", placeholder: "One thing worth your attention…" },
    { key: "intention2", placeholder: "A little time for yourself…" },
    { key: "intention3", placeholder: "Something to look forward to…" },
  ],
  highlights: [
    { key: "highlight1", placeholder: "A moment worth keeping…" },
    { key: "highlight2", placeholder: "Something that made you smile…" },
    { key: "highlight3", placeholder: "A small win counts, too…" },
  ],
} as const;

/** One derivation of the day for the editor, its capture and the icon. */
export function journalView(data: Data) {
  const withValues = (lines: readonly { key: LineKey; placeholder: string }[]) =>
    lines.map(line => ({ ...line, value: data[line.key] }));
  return {
    morningDone: data.morningDone,
    eveningDone: data.eveningDone,
    gratitudes: withValues(prompts.gratitudes),
    intentions: withValues(prompts.intentions),
    highlights: withValues(prompts.highlights),
    hasThought: Boolean(data.quote.trim() || data.quoteAuthor.trim()),
  };
}

export function initialView(data: { morningDone: boolean; eveningDone: boolean }): View {
  return data.morningDone ? (data.eveningDone ? "all" : "pm") : "am";
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function setDate(tx: Edit, date: string) {
  if (!calendarDate(date)) refuse("Enter the day as YYYY-MM-DD.");
  tx.fields.date.set(date);
}
export function setWritten(tx: Edit, period: Period, done: boolean) {
  if (period === "am") tx.fields.morningDone.set(done);
  else tx.fields.eveningDone.set(done);
}
export function writeLine(tx: Edit, line: LineKey, text: string) {
  tx.fields[line].set(text);
}
export function clearPeriod(tx: Edit, period: Period) {
  const keys: LineKey[] = period === "am"
    ? [...prompts.gratitudes, ...prompts.intentions].map(line => line.key)
    : prompts.highlights.map(line => line.key);
  keys.push(period === "am" ? "affirmation" : "lesson");
  for (const key of keys) tx.fields[key].set("");
  setWritten(tx, period, false);
  return keys.length;
}

export function calendarDate(value: string): CalendarDate | undefined {
  try {
    return parseDate(value);
  } catch {
    return undefined;
  }
}

export function displayDate(value: string): string {
  const date = new Date(`${value}T12:00:00`);
  return Number.isFinite(date.getTime())
    ? dateLabel(value, { weekday: "long", month: "short", day: "numeric", year: "numeric" })
    : value.trim() || "Choose a day";
}

export function dateString(value: DateValue): string {
  return new CalendarDate(value.year, value.month, value.day).toString();
}
