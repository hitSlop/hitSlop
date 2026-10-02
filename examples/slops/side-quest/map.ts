import type { Course, Quest } from "./schema";
import { byDue, cleared, dayValue, isoDay, span } from "./quest";

export function questLabel(quest: Quest): string { return quest.title.trim() || (quest.kind === "boss" ? "Boss" : "Quest"); }
export function courseLabel(course: Course): string { return course.code.trim() || course.name.trim() || "Course"; }

/** Where quests sit on the semester map, for the courses and today's date. */
export function mapGeometry(courses: readonly Course[], today: string) {
  const range = span(courses, today);
  function xOf(iso: string): number { return 3 + ((dayValue(iso, today) - range.start) / (range.end - range.start)) * 94; }
  function yOf(row: number, index: number, quest: Quest): number {
    const lane = 1000 / Math.max(1, courses.length);
    return lane * (row + 0.5) + (quest.kind === "boss" ? 0 : index % 2 ? lane * 0.16 : -lane * 0.16);
  }
  function points(course: Course, row: number) {
    return byDue(course.quests).map((quest, index) => ({ quest, x: xOf(quest.due) * 10, y: yOf(row, index, quest), up: quest.kind === "quest" && index % 2 === 0 }));
  }
  function trail(course: Course, row: number): string {
    const mid = (1000 / Math.max(1, courses.length)) * (row + 0.5);
    const stops = [{ x: 0, y: mid }, ...points(course, row), { x: 1000, y: mid }];
    let d = `M ${stops[0]!.x} ${stops[0]!.y}`;
    for (let i = 1; i < stops.length; i++) {
      const a = stops[i - 1]!, b = stops[i]!;
      const bend = (b.x - a.x) * 0.5;
      d += ` C ${a.x + bend} ${a.y}, ${b.x - bend} ${b.y}, ${b.x} ${b.y}`;
    }
    return d;
  }
  function shortDate(iso: string): string {
    return new Date(dayValue(iso, today)).toLocaleDateString(undefined, { month: "short", day: "numeric" });
  }
  function overdue(quest: Quest): boolean { return !cleared(quest) && quest.due < today; }
  const months = (() => {
    const marks: { x: number; label: string }[] = [];
    const cursor = new Date(range.start);
    cursor.setDate(1);
    cursor.setMonth(cursor.getMonth() + 1);
    while (cursor.getTime() < range.end) {
      marks.push({ x: xOf(isoDay(cursor)), label: cursor.toLocaleDateString(undefined, { month: "short" }) });
      cursor.setMonth(cursor.getMonth() + 1);
    }
    return marks;
  })();
  return { range, xOf, points, trail, months, shortDate, overdue };
}
