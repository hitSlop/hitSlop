import { refuse, type Tx, type Value } from "@hitslop/tinys";
import type schema from "./schema";
import type { days } from "./schema";
import { tones } from "./schema";

type Edit = Tx<typeof schema>;
export type Household = Value<typeof schema.descriptor>;
export type Person = Household["people"][number];
export type Chore = Household["chores"][number];
export type Day = (typeof days)[number];
export type Totals = { id: string; assigned: number; done: number };

// Points per person this week: assigned to them, and done by them.
export function weekTotals(people: readonly Person[], chores: readonly Chore[]): Totals[] {
  return people.map((person) => ({
    id: person.$id,
    assigned: chores.filter((chore) => chore.assignee === person.$id).reduce((sum, chore) => sum + chore.points, 0),
    done: chores.filter((chore) => chore.done && chore.doneBy === person.$id).reduce((sum, chore) => sum + chore.points, 0),
  }));
}

/** The door and its export derive totals, counts and owners from the same snapshot. */
export function householdView(data: Household) {
  const owners = new Map(data.people.map((person) => [person.$id, person]));
  return {
    totals: weekTotals(data.people, data.chores),
    open: data.chores.filter((chore) => !chore.done).length,
    week: (data.weeks ?? 0) + 1,
    unassigned: data.chores.filter((chore) => !owners.has(chore.assignee ?? "")),
    personById: (id: string | undefined) => (id === undefined ? undefined : owners.get(id)),
  };
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

function findPerson(current: Household, id: string) {
  return current.people.find((person) => person.$id === id) ?? refuse("No such person.");
}
function findChore(current: Household, id: string) {
  return current.chores.find((chore) => chore.$id === id) ?? refuse("No such chore.");
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function addPerson(tx: Edit, current: Household, name: string) {
  const trimmed = name.trim();
  if (!trimmed) refuse("Enter a name.");
  return tx.fields.people.insert({ name: trimmed, tone: tones[current.people.length % tones.length]!, score: 0 });
}

export function removePerson(tx: Edit, current: Household, person: string) {
  const target = findPerson(current, person);
  for (const chore of current.chores) {
    if (chore.assignee === target.$id) tx.fields.chores.item(chore.$id).assignee.clear();
    if (chore.doneBy === target.$id) tx.fields.chores.item(chore.$id).doneBy.clear();
  }
  tx.fields.people.remove(target.$id);
}

export function addChore(tx: Edit, current: Household, name: string) {
  const trimmed = name.trim();
  if (!trimmed) refuse("Enter a chore.");
  // Hand a new chore to whoever has the lightest load.
  const lightest = [...householdView(current).totals].sort((a, b) => a.assigned - b.assigned)[0];
  return tx.fields.chores.insert({ name: trimmed, ...(lightest ? { assignee: lightest.id } : {}) });
}

export function removeChore(tx: Edit, chore: string) {
  tx.fields.chores.remove(chore);
}

export function setDone(tx: Edit, current: Household, chore: string, done: boolean) {
  const target = findChore(current, chore);
  if (target.done === done) return;
  const row = tx.fields.chores.item(target.$id);
  const scorer = householdView(current).personById(done ? target.assignee : target.doneBy);
  row.done.set(done);
  if (done && target.assignee) row.doneBy.set(target.assignee);
  if (!done) row.doneBy.clear();
  if (scorer) tx.fields.people.item(scorer.$id).score.increment(done ? target.points : -target.points);
}

export function assignChore(tx: Edit, current: Household, chore: string, person: string | undefined) {
  const row = tx.fields.chores.item(findChore(current, chore).$id);
  if (person) row.assignee.set(findPerson(current, person).$id);
  else row.assignee.clear();
}

export function setDay(tx: Edit, chore: string, day: Day) {
  tx.fields.chores.item(chore).day.set(day);
}
export function setPoints(tx: Edit, chore: string, points: number) {
  tx.fields.chores.item(chore).points.set(points);
}
export function setRotates(tx: Edit, chore: string, rotates: boolean) {
  tx.fields.chores.item(chore).rotates.set(rotates);
}

// Hand each unassigned chore to whoever currently carries the fewest points.
export function dealOut(tx: Edit, current: Household) {
  const { totals, unassigned } = householdView(current);
  const load = new Map(totals.map((t) => [t.id, t.assigned]));
  if (!load.size) refuse("Add a person first.");
  for (const chore of [...unassigned].sort((a, b) => b.points - a.points)) {
    const [id] = [...load.entries()].sort((a, b) => a[1] - b[1])[0]!;
    tx.fields.chores.item(chore.$id).assignee.set(id);
    load.set(id, load.get(id)! + chore.points);
  }
  return unassigned.length;
}

// Clear done marks, rotate chores that rotate to the next person, and bump the week.
export function newWeek(tx: Edit, current: Household) {
  for (const chore of current.chores) {
    const row = tx.fields.chores.item(chore.$id);
    if (chore.done) { row.done.set(false); row.doneBy.clear(); }
    if (chore.rotates) {
      const next = nextPerson(current.people, chore.assignee);
      if (next) row.assignee.set(next);
    }
  }
  tx.fields.weeks.increment(1);
}
