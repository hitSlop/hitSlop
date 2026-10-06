import { type Day, type Stop, type StopTag } from "./schema";

export const TAGS = [
  { value: "flight", label: "Flight", code: "FLT" },
  { value: "hotel", label: "Hotel", code: "HTL" },
  { value: "dining", label: "Dining", code: "DINE" },
  { value: "train", label: "Transit", code: "RAIL" },
  { value: "explore", label: "Explore", code: "ACT" },
] as const;

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
  return new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3])).toLocaleDateString(undefined, { month: "short", day: "numeric" });
}
