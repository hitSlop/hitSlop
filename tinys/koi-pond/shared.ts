import { refuse, type RowRef, type Tx, type Value } from "@hitslop/tinys";
import { koiPatterns, maxKoi } from "./schema";
import type schema from "./schema";

export type Pond = Value<typeof schema.descriptor>;
export type KoiPattern = (typeof koiPatterns)[number];
type Edit = Tx<typeof schema>;
type Random = () => number;

export const maxName = 24;
export const koiNames = ["Mochi", "Pip", "Bean", "Noodle", "Sesame", "Dumpling", "Yuzu", "Tofu", "Biscuit", "Plum", "Nori", "Peach", "Button", "Miso", "Taro", "Sprout", "Pebble", "Kiki"];
export const patternLabels: Record<KoiPattern, string> = { kohaku: "Kohaku", showa: "Showa", ogon: "Ogon", asagi: "Asagi", tancho: "Tancho" };

export function list(items: string[]) {
  return items.length < 3 ? items.join(" & ") : `${items.slice(0, -1).join(", ")} & ${items.at(-1)}`;
}

/** The editor and the export derive names, counts and labels from the same snapshot. */
export function pondView(data: Pond) {
  const names = data.koi.map(koi => koi.name.trim() || "unnamed");
  return {
    count: data.koi.length,
    names: list(names),
    full: data.koi.length >= maxKoi,
    feedingLabel: `Fed ${data.feedings} ${data.feedings === 1 ? "time" : "times"}`,
  };
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.

/** Adds a koi with the first free name and a drawn pattern and size; returns its $id. */
export function addKoi(tx: Edit, current: Pond, random: Random, choice: { name?: string; pattern?: KoiPattern } = {}) {
  if (current.koi.length >= maxKoi) refuse("The pond is full. Nine is plenty!");
  const name = choice.name === undefined ? freeName(current) : cleanName(choice.name);
  const pattern = choice.pattern ?? koiPatterns[Math.min(koiPatterns.length - 1, Math.floor(random() * koiPatterns.length))]!;
  const size = Math.round((0.8 + random() * 0.4) * 100) / 100;
  return tx.fields.koi.insert({ name, pattern, size });
}

export function renameKoi(tx: Edit, current: Pond, koi: RowRef, name: string) {
  tx.fields.koi.item(findKoi(current, koi)).name.set(cleanName(name));
}

/** Removes a koi; returns the name it had. */
export function releaseKoi(tx: Edit, current: Pond, koi: RowRef) {
  const row = findKoi(current, koi);
  tx.fields.koi.remove(row);
  return row.name;
}

/** Counts one feeding; returns the new total. */
export function feedKoi(tx: Edit, current: Pond) {
  if (!current.koi.length) refuse("Add a koi first.");
  tx.fields.feedings.increment();
  return current.feedings + 1;
}

/** Stores a new painting seed (drawn when omitted, always different from the current one); returns it. */
export function repaintPond(tx: Edit, current: Pond, random: Random, seed?: number) {
  let next = seed ?? Math.min(999_999, Math.floor(random() * 1_000_000));
  if (!Number.isInteger(next) || next < 0 || next > 999_999) refuse("A seed is a whole number from 0 to 999999.");
  if (next === current.seed) {
    if (seed !== undefined) refuse("The pond is already painted with that seed.");
    next = (next + 1) % 1_000_000;
  }
  tx.fields.seed.set(next);
  return next;
}

function freeName(current: Pond) {
  const taken = new Set(current.koi.map(koi => koi.name));
  return koiNames.find(name => !taken.has(name)) ?? `Koi ${current.koi.length + 1}`;
}

function cleanName(name: string) {
  const clean = name.trim();
  if (!clean) refuse("Give the koi a name.");
  if (clean.length > maxName) refuse(`Koi names are at most ${maxName} characters.`);
  return clean;
}

function findKoi(current: Pond, koi: RowRef) {
  const id = typeof koi === "string" ? koi : koi.$id;
  return current.koi.find(row => row.$id === id) ?? refuse("No koi with that id is in the pond.");
}
