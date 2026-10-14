import { refuse, type Tx, type Value } from "hitslop";
import type schema from "./schema";
import { milestoneTypes } from "./schema";

type Edit = Tx<typeof schema>;
export type Semester = Value<typeof schema.descriptor>;
export type Milestone = Semester["milestones"][number];
export type MilestoneType = (typeof milestoneTypes)[number];

const FALLBACK_COLOR = "#4a5d6e";
const MONTH_NAMES = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

export const MILESTONE_TYPES: { type: MilestoneType; label: string; color: string }[] = [
  { type: "Exam", label: "Exam / Midterm", color: "#dc2626" },
  { type: "Paper", label: "Major Paper", color: "#d97706" },
  { type: "Project", label: "Project / Deliverable", color: "#4f46e5" },
  { type: "Presentation", label: "Presentation", color: "#0d9488" },
  { type: "Break", label: "Break / Recess", color: "#16a34a" },
  { type: "Deadline", label: "Administrative Deadline", color: "#4a5d6e" },
];

export function pad(n: number): string {
  return String(n).padStart(2, "0");
}

export function formatDateStr(year: number, monthIdx: number, day: number): string {
  return `${year}-${pad(monthIdx + 1)}-${pad(day)}`;
}

export function getTypeColor(type: MilestoneType): string {
  return MILESTONE_TYPES.find((item) => item.type === type)?.color ?? FALLBACK_COLOR;
}

/** True for a real calendar date written as YYYY-MM-DD. */
export function isDateString(value: string): boolean {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  if (!match) return false;
  const [year, month, day] = [Number(match[1]), Number(match[2]), Number(match[3])];
  const date = new Date(Date.UTC(year, month - 1, day));
  return date.getUTCFullYear() === year && date.getUTCMonth() === month - 1 && date.getUTCDate() === day;
}

export function daysBetween(from: string, to: string): number {
  return Math.round((Date.parse(`${to}T00:00:00Z`) - Date.parse(`${from}T00:00:00Z`)) / 86_400_000);
}

/** Four months from `startMonth` (YYYY-MM); an unreadable value shows the default term. */
function termMonths(startMonth: string) {
  const match = /^(\d{4})-(\d{2})$/.exec(startMonth);
  let year = match ? Number(match[1]) : 2026;
  let month = match && Number(match[2]) >= 1 && Number(match[2]) <= 12 ? Number(match[2]) - 1 : 8;
  return Array.from({ length: 4 }, () => {
    const entry = { year, month, name: MONTH_NAMES[month]!, days: new Date(Date.UTC(year, month + 1, 0)).getUTCDate() };
    month += 1;
    if (month === 12) { month = 0; year += 1; }
    return entry;
  });
}

/**
 * The calendar rail, ordering and colors for one snapshot. The editor and the capture both call
 * this; the capture passes no filter so it shows every course.
 */
export function semesterView(data: Semester, filterCourse = "ALL") {
  const sorted = [...data.milestones].sort((a, b) => a.date.localeCompare(b.date));
  const agenda = sorted.filter((m) => filterCourse === "ALL" || m.courseCode === filterCourse || m.courseCode === "ALL");
  const byDate = new Map<string, Milestone[]>();
  for (const milestone of data.milestones) byDate.set(milestone.date, [...(byDate.get(milestone.date) ?? []), milestone]);
  const colors = new Map(data.courses.map((course) => [course.code, course.colorHex]));
  return {
    months: termMonths(data.startMonth),
    agenda,
    on: (date: string) => byDate.get(date) ?? [],
    courseColor: (code: string) => colors.get(code) ?? FALLBACK_COLOR,
    /** The earliest milestone on or after `today` (YYYY-MM-DD). */
    next: (today: string) => sorted.find((m) => m.date >= today) ?? null,
  };
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function addMilestone(
  tx: Edit,
  current: Semester,
  input: { date: string; title: string; courseCode: string; type: MilestoneType },
) {
  const title = input.title.trim();
  if (!title) refuse("Enter a milestone title.");
  if (!isDateString(input.date)) refuse("Use a real date written as YYYY-MM-DD.");
  if (!current.courses.some((course) => course.code === input.courseCode)) refuse(`No course has the code ${input.courseCode}.`);
  return tx.fields.milestones.insert({ date: input.date, title, courseCode: input.courseCode, type: input.type, notes: "" });
}

export function moveMilestone(tx: Edit, milestone: string, date: string) {
  if (!isDateString(date)) refuse("Use a real date written as YYYY-MM-DD.");
  tx.fields.milestones.item(milestone).date.set(date);
}

export function removeMilestone(tx: Edit, milestone: string) {
  tx.fields.milestones.remove(milestone);
}
