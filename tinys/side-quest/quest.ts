import { refuse, type RowRef, type Tx, type Value } from "@hitslop/tinys";
import type schema from "./schema";
import { sprites, stickers, tapes } from "./schema";

export type Map = Value<typeof schema.descriptor>;
export type Course = Map["courses"][number];
export type Quest = Course["quests"][number];
export type Placed = Map["placed"][number];
export type Sticker = (typeof stickers)[number];
export type Tape = (typeof tapes)[number];
export type Kind = Quest["kind"];
type Edit = Tx<typeof schema>;

const DAY = 86_400_000;
const ISO = /^(\d{4})-(\d{2})-(\d{2})$/;

export const emoji: Record<Sticker, string> = {
  star: "⭐", heart: "💖", bolt: "⚡", fire: "🔥", crown: "👑", ghost: "👻", cherry: "🍒", sparkle: "✨", frog: "🐸", rainbow: "🌈",
};
export const spriteFace = { cat: "🐱", frog: "🐸", bunny: "🐰", ghost: "👻", alien: "👾", bear: "🐻" } as const;

export function isoDay(date: Date): string {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}

/** Local-midnight epoch for an ISO day; an invalid day falls back to `fallback` (NaN without one). */
export function dayValue(iso: string, fallback = ""): number {
  const match = ISO.exec(iso) ?? ISO.exec(fallback);
  return match ? new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3])).getTime() : NaN;
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

/** The map header and the export derive the boss, the tallies and the loot from the same snapshot. */
export function questView(data: Map, today: string) {
  const allQuests = data.courses.flatMap((course) => course.quests);
  const clearedCount = allQuests.filter(cleared).length;
  const earned = clearedCount + data.sideQuests.filter((side) => side.done).length;
  return {
    allQuests, clearedCount, earned,
    loot: Math.max(0, earned - data.placed.length),
    boss: nextBoss(data.courses, today),
  };
}
const lootCount = (data: Map) => questView(data, "").loot;

// Each edit is written once: the page runs it with doc.change, a command with its tx.
function findQuest(current: Map, quest: RowRef): { course: Course; quest: Quest } {
  const id = typeof quest === "string" ? quest : quest.$id;
  for (const course of current.courses) {
    const found = course.quests.find((item) => item.$id === id);
    if (found) return { course, quest: found };
  }
  return refuse("No such quest.");
}
function checkDue(due: string) {
  const match = ISO.exec(due);
  if (!match || isoDay(new Date(dayValue(due))) !== due) refuse("Use a YYYY-MM-DD due date.");
}

export function addCourse(tx: Edit, current: Map) {
  if (current.courses.length >= 6) refuse("The map holds six courses.");
  const used = new Set(current.courses.map((course) => course.tape));
  const tape = tapes.find((item) => !used.has(item)) ?? tapes[current.courses.length % tapes.length]!;
  return tx.fields.courses.insert({ code: "NEW 100", name: "", tape, quests: [] });
}
export function removeCourse(tx: Edit, course: RowRef) {
  tx.fields.courses.remove(course);
}
export function cycleTape(tx: Edit, course: Course) {
  tx.at(course).tape.set(tapes[(tapes.indexOf(course.tape) + 1) % tapes.length]!);
}
export function addQuest(tx: Edit, course: RowRef, kind: Kind, due: string, title = "") {
  checkDue(due);
  const name = title.trim() || (kind === "boss" ? "Exam" : "New quest");
  return tx.fields.courses.item(course).quests.insert(kind === "boss"
    ? { title: name, kind, due, maxHp: 6 }
    : { title: name, kind, due });
}
export function removeQuest(tx: Edit, current: Map, quest: RowRef) {
  const { course, quest: row } = findQuest(current, quest);
  tx.fields.courses.item(course).quests.remove(row);
}
/** Clearing a plain quest earns its sticker; bosses are cleared by study sessions instead. */
export function setDone(tx: Edit, current: Map, quest: RowRef, done: boolean) {
  const found = findQuest(current, quest);
  if (found.quest.kind === "boss") refuse("Bosses are cleared by logging study sessions.");
  const item = tx.fields.courses.item(found.course).quests.item(found.quest);
  item.done.set(done);
  if (done) item.sticker.set(lootFor(found.quest.$id));
  else item.sticker.clear();
}
/** One study session (+1) knocks a point off the boss; undo (-1) gives it back. Returns the HP left. */
export function hit(tx: Edit, current: Map, quest: RowRef, by: 1 | -1) {
  const found = findQuest(current, quest);
  const currentHp = hp(found.quest);
  if (found.quest.kind !== "boss") refuse("Only bosses take study sessions.");
  if (found.quest.hits === null || currentHp === null) refuse("This boss's HP is unavailable.");
  if (by > 0 && currentHp === 0) refuse("This boss is already defeated.");
  if (by < 0 && found.quest.hits <= 0) refuse("No study sessions to undo.");
  const item = tx.fields.courses.item(found.course).quests.item(found.quest);
  item.hits.increment(by);
  if (by > 0 && currentHp === 1) item.sticker.set(lootFor(found.quest.$id));
  else if (by < 0 && currentHp === 0) { item.sticker.clear(); item.done.set(false); }
  return Math.max(0, currentHp - by);
}
export function setMaxHp(tx: Edit, quest: Quest, value: number) {
  if (!Number.isFinite(value)) return;
  tx.at(quest).maxHp.set(Math.min(50, Math.max(1, Math.round(value))));
}
export function setDue(tx: Edit, current: Map, quest: RowRef, due: string) {
  checkDue(due);
  const found = findQuest(current, quest);
  tx.fields.courses.item(found.course).quests.item(found.quest).due.set(due);
}
export function addSideQuest(tx: Edit, title: string) {
  const name = title.trim();
  if (!name) refuse("Enter a side quest.");
  return tx.fields.sideQuests.insert({ title: name });
}
export function setSideDone(tx: Edit, side: RowRef, done: boolean) {
  tx.fields.sideQuests.item(side).done.set(done);
}
export function placeSticker(tx: Edit, current: Map, sticker: Sticker) {
  if (lootCount(current) === 0) refuse("No loot to place yet.");
  const count = current.placed.length;
  return tx.fields.placed.insert({ sticker, x: 0.2 + ((count * 0.137) % 0.6), y: 0.15 + ((count * 0.291) % 0.7), turn: ((count * 37) % 30) - 15 });
}
export function moveSticker(tx: Edit, placed: RowRef, x: number, y: number) {
  const item = tx.fields.placed.item(placed);
  item.x.set(x);
  item.y.set(y);
}
export function peelSticker(tx: Edit, placed: RowRef) {
  tx.fields.placed.remove(placed);
}
export function cycleSprite(tx: Edit, current: Map) {
  tx.fields.player.sprite.set(sprites[(sprites.indexOf(current.player.sprite) + 1) % sprites.length]!);
}
