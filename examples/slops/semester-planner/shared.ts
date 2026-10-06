import { type MilestoneType } from "./schema";

export const MILESTONE_TYPES: { type: MilestoneType; label: string; color: string }[] = [
  { type: "Exam", label: "Exam / Midterm", color: "#dc2626" },
  { type: "Paper", label: "Major Paper", color: "#d97706" },
  { type: "Project", label: "Project / Deliverable", color: "#4f46e5" },
  { type: "Presentation", label: "Presentation", color: "#0d9488" },
  { type: "Break", label: "Break / Recess", color: "#16a34a" },
  { type: "Deadline", label: "Administrative Deadline", color: "#4a5d6e" },
];

export const MONTHS = [
  { year: 2026, month: 8, name: "September", days: 30 },
  { year: 2026, month: 9, name: "October", days: 31 },
  { year: 2026, month: 10, name: "November", days: 30 },
  { year: 2026, month: 11, name: "December", days: 31 },
];

export function pad(n: number): string {
  return String(n).padStart(2, "0");
}

export function formatDateStr(year: number, monthIdx: number, day: number): string {
  return `${year}-${pad(monthIdx + 1)}-${pad(day)}`;
}

export function getTypeColor(type: MilestoneType): string {
  return MILESTONE_TYPES.find((item) => item.type === type)?.color || "#4a5d6e";
}
