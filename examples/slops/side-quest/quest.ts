import { stickers, type Course, type Quest, type Sticker } from "./schema";

const DAY = 86_400_000;

export const emoji: Record<Sticker, string> = {
  star: "⭐", heart: "💖", bolt: "⚡", fire: "🔥", crown: "👑", ghost: "👻", cherry: "🍒", sparkle: "✨", frog: "🐸", rainbow: "🌈",
};
export const spriteFace = { cat: "🐱", frog: "🐸", bunny: "🐰", ghost: "👻", alien: "👾", bear: "🐻" } as const;

export function isoDay(date: Date): string {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}

/** Local-midnight epoch for an ISO day; invalid dates fall back to today. */
export function dayValue(iso: string, today = isoDay(new Date())): number {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso) ?? /^(\d{4})-(\d{2})-(\d{2})$/.exec(today)!;
  return new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3])).getTime();
}

export function daysBetween(from: string, to: string): number {
  return Math.round((dayValue(to, from) - dayValue(from)) / DAY);
}

export function addDays(iso: string, days: number): string {
  const date = new Date(dayValue(iso));
  date.setDate(date.getDate() + days);
  return isoDay(date);
}

export const hp = (quest: Quest) => quest.hits === null ? null : Math.max(0, quest.maxHp - quest.hits);
export const cleared = (quest: Quest) => quest.done || (quest.kind === "boss" && hp(quest) === 0);

export function byDue(quests: readonly Quest[]): Quest[] {
  return [...quests].sort((a, b) => a.due.localeCompare(b.due) || a.$id.localeCompare(b.$id));
}

export function nextBoss(courses: readonly Course[], today: string): { course: Course; quest: Quest } | undefined {
  let best: { course: Course; quest: Quest } | undefined;
  for (const course of courses)
    for (const quest of course.quests)
      if (quest.kind === "boss" && !cleared(quest) && quest.due >= today && (!best || quest.due < best.quest.due))
        best = { course, quest };
  return best;
}

/** Timeline span covering today and every due date, padded by a few days on each side. */
export function span(courses: readonly { quests: readonly { due: string }[] }[], today: string): { start: number; end: number } {
  let start = dayValue(today);
  let end = start;
  for (const course of courses)
    for (const quest of course.quests) {
      const value = dayValue(quest.due, today);
      start = Math.min(start, value);
      end = Math.max(end, value);
    }
  return { start: start - 5 * DAY, end: Math.max(end, start + 28 * DAY) + 5 * DAY };
}

export function when(days: number): string {
  if (days === 0) return "today";
  if (days === 1) return "tomorrow";
  if (days === -1) return "yesterday";
  return days > 0 ? `in ${days} days` : `${-days} days ago`;
}

/** A stable, varied sticker for a cleared quest. */
export function lootFor(id: string): Sticker {
  let hash = 0;
  for (const char of id) hash = (hash * 31 + char.charCodeAt(0)) | 0;
  return stickers[Math.abs(hash) % stickers.length]!;
}
