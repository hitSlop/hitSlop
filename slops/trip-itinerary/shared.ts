import { dateLabel } from "../_shared/format";
import { refuse, type RowRef, type Tx } from "hitslop";
import type doc from "./schema";
import type { stopTags } from "./schema";

export type Trip = typeof doc.current;
export type Day = Trip["days"][number];
export type Stop = Day["events"][number];
export type StopTag = (typeof stopTags)[number];
type Edit = Tx<typeof doc>;

export const TAGS = [
  { value: "flight", label: "Flight", code: "FLT" },
  { value: "hotel", label: "Hotel", code: "HTL" },
  { value: "dining", label: "Dining", code: "DINE" },
  { value: "train", label: "Transit", code: "RAIL" },
  { value: "explore", label: "Explore", code: "ACT" },
] as const;

export const DEFAULT_DAY_SUBTITLE = "Exploration & Activities";

export function orderDays(days: readonly Day[]): Day[] {
  return days
    .map((day, index) => ({ day, index }))
    .sort((a, b) => {
      if (a.day.date && b.day.date && a.day.date !== b.day.date) return a.day.date.localeCompare(b.day.date);
      if (a.day.date && !b.day.date) return -1;
      if (!a.day.date && b.day.date) return 1;
      return a.index - b.index;
    })
    .map((item) => item.day);
}

export function orderStops(events: readonly Stop[]): Stop[] {
  return events
    .map((event, index) => ({ event, index }))
    .sort((a, b) => {
      if (a.event.time !== b.event.time) return (a.event.time || "").localeCompare(b.event.time || "");
      return a.index - b.index;
    })
    .map((item) => item.event);
}

export function tagCode(id: StopTag): string {
  return TAGS.find((tag) => tag.value === id)?.code ?? id.toUpperCase();
}

export function formatDate(iso: string): string {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso);
  if (!match) return iso;
  return dateLabel(new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3])),  { month: "short", day: "numeric" });
}

export function nextDate(from: string): string {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(from);
  if (!match) return "";
  const date = new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3]) + 1);
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}

/** The booklet and the export derive days, stops and counts from the same snapshot. */
export function itineraryView(data: Trip) {
  const days = orderDays(data.days);
  const active = data.days.find((day) => day.dayKey === data.selectedDay) ?? data.days[0];
  const packed = data.stubItems.filter((item) => item.done).length;
  return {
    tabs: days.map((day) => ({ day, done: day.events.filter((event) => event.done).length, total: day.events.length })),
    schedule: days.map((day) => ({ day, stops: orderStops(day.events) })),
    active,
    activeStops: active ? orderStops(active.events) : [],
    packed,
    packedTotal: data.stubItems.length,
    boarded: data.stubItems.length > 0 && packed === data.stubItems.length,
  };
}

function dayOf(data: Trip, dayKey: string): Day {
  const day = data.days.find((entry) => entry.dayKey === dayKey);
  if (!day) refuse("No day has that key.");
  return day;
}

function locateStop(data: Trip, stop: RowRef): { day: Day; stop: Stop } {
  const id = typeof stop === "string" ? stop : stop.$id;
  for (const day of data.days) {
    const found = day.events.find((event) => event.$id === id);
    if (found) return { day, stop: found };
  }
  return refuse("No stop has that $id.");
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function addStop(tx: Edit, data: Trip, dayKey: string, input: { time: string; title: string; location: string; tag: StopTag }) {
  const title = input.title.trim();
  if (!title) refuse("Enter a stop title.");
  return tx.at(dayOf(data, dayKey)).events.insert({
    time: input.time || "10:00",
    title,
    location: input.location.trim(),
    tag: input.tag,
  });
}
export function setStopDone(tx: Edit, data: Trip, stop: RowRef, done: boolean) {
  tx.at(locateStop(data, stop).stop).done.set(done);
}
export function setStopTime(tx: Edit, stop: Stop, time: string) {
  tx.at(stop).time.set(time);
}
export function removeStop(tx: Edit, data: Trip, stop: RowRef) {
  const found = locateStop(data, stop);
  tx.at(found.day).events.remove(found.stop.$id);
}
export function addDay(tx: Edit, data: Trip, dayKey: string, subtitle: string) {
  const ordered = orderDays(data.days);
  const last = ordered[ordered.length - 1];
  tx.fields.days.insert({
    dayKey,
    title: `DAY ${String(data.days.length + 1).padStart(2, "0")}`,
    subtitle: subtitle.trim() || DEFAULT_DAY_SUBTITLE,
    date: last ? nextDate(last.date) : "",
    events: [],
  });
  tx.fields.selectedDay.set(dayKey);
  return dayKey;
}
export function setDayDate(tx: Edit, day: Day, date: string) {
  tx.at(day).date.set(date);
}
export function selectDay(tx: Edit, data: Trip, dayKey: string) {
  if (data.days.some((day) => day.dayKey === dayKey)) tx.fields.selectedDay.set(dayKey);
}
export function removeDay(tx: Edit, data: Trip, day: Day) {
  if (data.days.length <= 1) return;
  const next = data.days.find((entry) => entry.$id !== day.$id);
  tx.fields.days.remove(day.$id);
  if (data.selectedDay === day.dayKey && next) tx.fields.selectedDay.set(next.dayKey);
}
export function addPackingItem(tx: Edit, text: string) {
  const trimmed = text.trim();
  if (!trimmed) refuse("Enter a packing item.");
  return tx.fields.stubItems.insert({ text: trimmed });
}
export function setPackingDone(tx: Edit, item: RowRef, done: boolean) {
  tx.fields.stubItems.item(item).done.set(done);
}
export function removePackingItem(tx: Edit, item: RowRef) {
  tx.fields.stubItems.remove(item);
}
