import { defineDocument, s, type Value } from "@hitslop/document";

export const DAYS = ["MON", "TUE", "WED", "THU", "FRI"] as const;
export type DayKey = (typeof DAYS)[number];

export const COLORS = ["sage", "slate", "amber", "terracotta", "indigo", "rose", "teal"] as const;
export type SubjectColor = (typeof COLORS)[number];

const schema = defineDocument({
  studentName: s.text(),
  term: s.text(),
  homeroom: s.text(),
  locker: s.text(),
  periods: s.list(s.object({
    periodKey: s.string(),
    name: s.text(),
    start: s.string(),
    end: s.string(),
  })),
  classes: s.list(s.object({
    periodKey: s.string(),
    day: s.enum(DAYS),
    subject: s.text(),
    room: s.text(),
    teacher: s.text(),
    color: s.enum(COLORS),
  })),
  activities: s.list(s.object({
    day: s.enum(DAYS),
    start: s.string(),
    end: s.string(),
    title: s.text(),
    location: s.text(),
  })),
});

export type Schedule = Value<typeof schema.fields.node>;
export type PeriodSlot = Schedule["periods"][number];
export type ClassEntry = Schedule["classes"][number];
export type ActivityEntry = Schedule["activities"][number];
export default schema;

export function isDay(value: string): value is DayKey {
  return (DAYS as readonly string[]).includes(value);
}

export function isColor(value: string): value is SubjectColor {
  return (COLORS as readonly string[]).includes(value);
}

export function isBreakName(name: string): boolean {
  const label = name.toLowerCase();
  return label.includes("lunch") || label.includes("advisory") || label.includes("break") || label.includes("recess");
}
