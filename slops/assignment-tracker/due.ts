import type { DateValue } from "@internationalized/date";
import { refuse, type RowRef, type Tx } from "hitslop";
import type doc from "./schema";

type Edit = Tx<typeof doc>;
export type Data = typeof doc.current;
export type Assignment = Data["assignments"][number];

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

/** The only clock read in this module. Pages call it; edits and commands never do. */
export function todayISO(): string {
  const date = new Date();
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}

export function parseISO(iso: string): Date | null {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(iso) || Number(iso.slice(0, 4)) < 1) return null;
  const date = new Date(`${iso}T12:00:00Z`);
  return Number.isFinite(date.getTime()) && date.toISOString().slice(0, 10) === iso ? date : null;
}

export function daysUntil(iso: string, reference: string): number | null {
  const target = parseISO(iso);
  const today = parseISO(reference);
  if (!target || !today) return null;
  return Math.round((target.getTime() - today.getTime()) / 86400000);
}

export function formatShort(iso: string): string {
  const d = parseISO(iso);
  return d
    ? `${["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"][d.getUTCMonth()]} ${d.getUTCDate()}`
    : "Choose date";
}

export function relativeDue(iso: string, reference: string): { label: string; urgency: Urgency } {
  const days = daysUntil(iso, reference);
  if (days === null) return { label: "Choose date", urgency: "undated" };
  if (days < 0) return { label: `${Math.abs(days)}d overdue`, urgency: "overdue" };
  if (days === 0) return { label: "Due today", urgency: "today" };
  if (days === 1) return { label: "Tomorrow", urgency: "soon" };
  if (days <= 7) return { label: `In ${days} days`, urgency: "soon" };
  return { label: formatShort(iso), urgency: "later" };
}

export function groupAssignments(items: readonly Assignment[], reference: string) {
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

/** The editor and the export derive the same lists and counts from one snapshot. */
export function trackerView(data: Data, day: string) {
  const active = data.assignments.filter((item) => !item.completed);
  const completed = data.assignments.filter((item) => item.completed);
  const dueToday = active.filter((item) => relativeDue(item.dueDate, day).urgency === "today").length;
  const overdue = active.filter((item) => relativeDue(item.dueDate, day).urgency === "overdue").length;
  return {
    active,
    completed,
    dueToday,
    overdue,
    readout: overdue ? `${overdue} overdue` : dueToday ? `${dueToday} due today` : active.length ? `${active.length} to do` : "All caught up",
    todo: groupAssignments(active, day),
    done: groupAssignments(completed, day),
    all: groupAssignments(data.assignments, day),
  };
}

export type AssignmentInput = {
  courseCode: string;
  title: string;
  dueDate: string;
  category: string;
  points: number;
  completed: boolean;
  notes: string;
};

function checked(input: AssignmentInput, oldDueDate?: string): AssignmentInput {
  const title = input.title.trim();
  if (!title) refuse("Give this assignment a title.");
  if (!Number.isFinite(input.points) || input.points < 0) refuse("Points must be zero or greater.");
  if (input.dueDate && input.dueDate !== oldDueDate && !parseISO(input.dueDate)) {
    refuse("Choose a valid due date (YYYY-MM-DD) or leave it undated.");
  }
  return { ...input, title };
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function addAssignment(tx: Edit, input: AssignmentInput) {
  return tx.fields.assignments.insert(checked(input));
}

export function updateAssignment(tx: Edit, current: Data, id: RowRef, input: AssignmentInput) {
  const old = current.assignments.find((item) => item.$id === id);
  if (!old) refuse("No assignment has that id.");
  const next = checked(input, old.dueDate);
  const row = tx.fields.assignments.item(id);
  row.courseCode.set(next.courseCode);
  row.title.set(next.title);
  row.dueDate.set(next.dueDate);
  row.category.set(next.category);
  row.points.set(next.points);
  row.completed.set(next.completed);
  row.notes.set(next.notes);
}

export function setCompleted(tx: Edit, id: RowRef, completed: boolean) {
  tx.fields.assignments.item(id).completed.set(completed);
}

export function setDueDate(tx: Edit, current: Data, id: RowRef, dueDate: string) {
  const old = current.assignments.find((item) => item.$id === id);
  if (!old) refuse("No assignment has that id.");
  if (dueDate && dueDate !== old.dueDate && !parseISO(dueDate)) {
    refuse("Choose a valid due date (YYYY-MM-DD) or leave it undated.");
  }
  tx.fields.assignments.item(id).dueDate.set(dueDate);
}

export function removeAssignment(tx: Edit, id: RowRef) {
  return tx.fields.assignments.remove(id);
}
