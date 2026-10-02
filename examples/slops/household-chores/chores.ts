import type { Chore, Person } from "./schema";

export type Totals = { id: string; assigned: number; done: number };

// Points per person this week: assigned to them, and done by them.
export function weekTotals(people: readonly Person[], chores: readonly Chore[]): Totals[] {
  return people.map((person) => ({
    id: person.$id,
    assigned: chores.filter((chore) => chore.assignee === person.$id).reduce((sum, chore) => sum + chore.points, 0),
    done: chores.filter((chore) => chore.done && chore.doneBy === person.$id).reduce((sum, chore) => sum + chore.points, 0),
  }));
}

export function initials(name: string): string {
  const words = name.trim().split(/\s+/).filter(Boolean);
  if (!words.length) return "?";
  return (words.length === 1 ? words[0]!.slice(0, 2) : words[0]![0]! + words[1]![0]!).toUpperCase();
}

export function nextPerson(people: readonly Person[], currentId: string | undefined): string | undefined {
  if (!people.length) return undefined;
  const at = people.findIndex((person) => person.$id === currentId);
  return people[(at + 1) % people.length]!.$id;
}

// SVG arc path for a ring segment between two angles (radians, 0 = top, clockwise).
export function ringArc(cx: number, cy: number, outer: number, inner: number, from: number, to: number): string {
  const sweep = Math.min(to - from, Math.PI * 2 - 0.0001);
  const end = from + sweep;
  const pt = (radius: number, angle: number) => [cx + radius * Math.sin(angle), cy - radius * Math.cos(angle)] as const;
  const [x1, y1] = pt(outer, from), [x2, y2] = pt(outer, end), [x3, y3] = pt(inner, end), [x4, y4] = pt(inner, from);
  const large = sweep > Math.PI ? 1 : 0;
  return `M${x1} ${y1}A${outer} ${outer} 0 ${large} 1 ${x2} ${y2}L${x3} ${y3}A${inner} ${inner} 0 ${large} 0 ${x4} ${y4}Z`;
}
