import type { MorningPages } from "./schema";

export const TARGET = 750;

export function formatDisplayDate(key: string): string {
  const [y, m, d] = key.split("-").map(Number);
  return new Date(y, (m ?? 1) - 1, d).toLocaleDateString(undefined, {
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

/** The editor and captures count the open page the same way. */
export function pageView(data: MorningPages) {
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
