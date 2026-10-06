import { CalendarDate, getLocalTimeZone, today as getTodayDate, type DateValue } from "@internationalized/date";
import type { Assignment } from "./schema";

export type Urgency = "overdue" | "today" | "soon" | "later" | "undated";
export const STACKS = [
  { id: "overdue", label: "Overdue" },
  { id: "today", label: "Due today" },
  { id: "soon", label: "Next 7 days" },
  { id: "later", label: "Later" },
  { id: "undated", label: "Needs a date" },
  { id: "done", label: "Completed" },
] as const;

export function formatISO(date: DateValue): string {
  return `${String(date.year).padStart(4, "0")}-${String(date.month).padStart(2, "0")}-${String(date.day).padStart(2, "0")}`;
}

export function todayISO(): string {
  return formatISO(getTodayDate(getLocalTimeZone()));
}

export function addDaysISO(days: number): string {
  return formatISO(getTodayDate(getLocalTimeZone()).add({ days }));
}

export function parseISO(iso: string): CalendarDate | null {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(iso)) return null;
  const [y, m, d] = iso.split("-").map(Number);
  if (y < 1 || m < 1 || m > 12 || d < 1 || d > 31) return null;
  try {
    const date = new CalendarDate(y, m, d);
    return formatISO(date) === iso ? date : null;
  } catch {
    return null;
  }
}

export function daysUntil(iso: string, reference = todayISO()): number | null {
  const target = parseISO(iso);
  const today = parseISO(reference);
  if (!target || !today) return null;
  return target.compare(today);
}

export function formatShort(iso: string): string {
  const d = parseISO(iso);
  return d
    ? `${["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"][d.month - 1]} ${d.day}`
    : "Choose date";
}

export function relativeDue(iso: string, reference = todayISO()): { label: string; urgency: Urgency } {
  const days = daysUntil(iso, reference);
  if (days === null) return { label: "Choose date", urgency: "undated" };
  if (days < 0) return { label: `${Math.abs(days)}d overdue`, urgency: "overdue" };
  if (days === 0) return { label: "Due today", urgency: "today" };
  if (days === 1) return { label: "Tomorrow", urgency: "soon" };
  if (days <= 7) return { label: `In ${days} days`, urgency: "soon" };
  return { label: formatShort(iso), urgency: "later" };
}

export function groupAssignments(items: readonly Assignment[], reference = todayISO()) {
  return STACKS.map((stack) => ({
    ...stack,
    items: items
      .filter((item) => (item.completed ? "done" : relativeDue(item.dueDate, reference).urgency) === stack.id)
      .sort((a, b) => {
        const aDate = daysUntil(a.dueDate, reference) ?? Infinity;
        const bDate = daysUntil(b.dueDate, reference) ?? Infinity;
        return (aDate === bDate ? 0 : aDate - bDate) || b.points - a.points || a.title.localeCompare(b.title);
      }),
  })).filter((stack) => stack.items.length);
}

export function msUntilNextDay(now: Date): number {
  const next = new Date(now);
  next.setHours(24, 0, 0, 0);
  return Math.max(1, next.getTime() - now.getTime() + 50);
}
