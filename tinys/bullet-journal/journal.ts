import { refuse, type RowRef, type Tx, type Value } from "@hitslop/tinys";
import type schema from "./schema";
import { nextSignifier, type Signifier } from "./signifiers";

export type Journal = Value<typeof schema.descriptor>;
type Edit = Tx<typeof schema>;

/** The page and the export derive their rows and counts from the same snapshot. */
export function journalView(data: Journal) {
  return {
    entries: data.entries,
    openCount: data.entries.filter((entry) => entry.type === "task" || entry.type === "scheduled").length,
    days: data.monthlyLog,
  };
}

export function isDayOfMonth(value: unknown): value is number {
  return typeof value === "number" && Number.isInteger(value) && value >= 1 && value <= 31;
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function addEntry(tx: Edit, input: { text: string; type: Signifier; star: boolean }) {
  const text = input.text.trim();
  if (!text) refuse("Enter an entry first.");
  return tx.fields.entries.insert({ type: input.type, star: input.star, text });
}
export function cycleSignifier(tx: Edit, current: Journal, entry: RowRef) {
  const id = typeof entry === "string" ? entry : entry.$id;
  const row = current.entries.find((item) => item.$id === id);
  if (!row) refuse("That entry is no longer on the page.");
  const type = nextSignifier(row.type);
  tx.fields.entries.item(entry).type.set(type);
  return type;
}
export function setPriority(tx: Edit, entry: RowRef, star: boolean) {
  tx.fields.entries.item(entry).star.set(star);
}
export function removeEntry(tx: Edit, entry: RowRef) {
  tx.fields.entries.remove(entry);
}
export function addMonthDay(tx: Edit, current: Journal) {
  const last = current.monthlyLog.at(-1);
  const day = Math.min(31, (last?.day ?? 0) + 1);
  return tx.fields.monthlyLog.insert({ day, weekday: "", text: "" });
}
export function setDay(tx: Edit, row: RowRef, day: number) {
  if (!isDayOfMonth(day)) refuse("Enter a day from 1 to 31.");
  tx.fields.monthlyLog.item(row).day.set(day);
}
export function removeMonthDay(tx: Edit, row: RowRef) {
  tx.fields.monthlyLog.remove(row);
}
