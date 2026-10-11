import { refuse, type RowRef, type Tx } from "hitslop";
import type doc from "./schema";
export type Journal = typeof doc.current;
type Edit = Tx<typeof doc>;

const WEEKDAYS = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"] as const;

/** The journal and its export derive the rows and count from the same snapshot. */
export function moodView(data: Journal) {
  return {
    entries: data.entries,
    count: data.entries.length,
  };
}

function dayLabel(at: Date): string {
  return `${WEEKDAYS[at.getDay()]} ${at.getDate()}`;
}

function isLevel(value: number): boolean {
  return Number.isInteger(value) && value >= 1 && value <= 5;
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function addEntry(tx: Edit, current: Journal, input: { mood: number; energy: number; note: string }, at: Date) {
  const note = input.note.trim();
  if (!note) refuse("Write one line.");
  if (!isLevel(input.mood) || !isLevel(input.energy)) refuse("Choose a mood and energy from 1 to 5.");
  const row = { day: dayLabel(at), mood: input.mood, energy: input.energy, note };
  const first = current.entries[0];
  return first ? tx.fields.entries.insert(row, { before: first.$id }) : tx.fields.entries.insert(row);
}
export function removeEntry(tx: Edit, entry: RowRef) {
  tx.fields.entries.remove(entry);
}
export function setPrompt(tx: Edit, prompt: string) {
  tx.fields.prompt.set(prompt);
}
