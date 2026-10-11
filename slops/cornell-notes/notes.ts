import { refuse, type Tx } from "hitslop";
import type doc from "./schema";
import type { themes } from "./schema";

export type Notes = typeof doc.current;
export type Theme = (typeof themes)[number];
type Edit = Tx<typeof doc>;

export const DEFAULT_CUE = "New Question or Key Term";
export const DEFAULT_NOTES = "- Detailed notes and supporting facts...";

/** The editor and the export derive their items and mode from the same snapshot. */
export function notesView(data: Notes) {
  return {
    items: data.items,
    total: data.items.length,
    dark: data.theme === "dark",
  };
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function addItem(tx: Edit, cue: string = DEFAULT_CUE, notes: string = DEFAULT_NOTES) {
  return tx.fields.items.insert({ cue, notes });
}
export function removeItem(tx: Edit, id: string) {
  tx.fields.items.remove(id);
}
export function setTheme(tx: Edit, theme: Theme) {
  tx.fields.theme.set(theme);
}
export function setDate(tx: Edit, date: string) {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(date)) refuse("Use a date like 2026-09-02.");
  tx.fields.date.set(date);
}
