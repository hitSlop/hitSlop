import { dateLabel } from "../_shared/format";
import { refuse, type Tx, type Value } from "hitslop";
import type schema from "./schema";

export type Data = Value<typeof schema.descriptor>;
export type Page = Data["entries"][string];
type Edit = Tx<typeof schema>;

export const TARGET = 750;

/** Local calendar date as YYYY-MM-DD; the caller supplies the instant. */
export function dateKey(d: Date): string {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

export function addDays(key: string, days: number): string {
  const [y, m, d] = key.split("-").map(Number);
  const date = new Date(y, (m ?? 1) - 1, d);
  date.setDate(date.getDate() + days);
  return dateKey(date);
}

export function formatDisplayDate(key: string): string {
  const [y, m, d] = key.split("-").map(Number);
  return dateLabel(new Date(y, (m ?? 1) - 1, d), {
    weekday: "long",
    month: "long",
    day: "numeric",
    year: "numeric",
  });
}

export function countWords(text: string): number {
  const trimmed = text.trim();
  return trimmed === "" ? 0 : trimmed.split(/\s+/).length;
}

/** The editor, the export and the icon count the open page the same way. */
export function pageView(data: Data) {
  const active = data.entries[data.currentKey];
  const wordsCount = countWords(active?.text ?? "");
  return {
    active,
    wordsCount,
    progressPct: Math.min(100, Math.round((wordsCount / TARGET) * 100)),
    page1Done: wordsCount >= 250,
    page2Done: wordsCount >= 500,
    page3Done: wordsCount >= TARGET,
  };
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.

/** Opens a day, creating its page when absent, in one write. */
export function openPage(tx: Edit, data: Data, key: string) {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(key)) refuse("Use a date as YYYY-MM-DD.");
  if (!data.entries[key]) tx.fields.entries.put(key, { date: key, text: "", completedAt: "" });
  tx.fields.currentKey.set(key);
  return key;
}

/** Stamps the page the first time it reaches the target; later edits keep the first stamp. */
export function markFinished(tx: Edit, page: Page, words: number, at: Date) {
  if (words < TARGET || page.completedAt) return;
  tx.at(page).completedAt.set(dateLabel(at, { hour: "2-digit", minute: "2-digit" }));
}

/** Replaces the open page's text and returns its word count. */
export function writePage(tx: Edit, data: Data, text: string, at: Date) {
  const page = data.entries[data.currentKey];
  if (!page) refuse("Open a day first.");
  tx.at(page).text.set(text);
  const words = countWords(text);
  markFinished(tx, page, words, at);
  return words;
}
