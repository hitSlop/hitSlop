import { refuse, type RowRef, type Tx, type Value } from "hitslop";
import type schema from "./schema";
import { PRESETS } from "./presets";
export type Data = Value<typeof schema.descriptor>;
type Edit = Tx<typeof schema>;

/**
 * The tag and the export derive their totals, sections and empty state from the same snapshot.
 * `filter` is the tag's view only: "all", "remaining" or a category key. The export never filters.
 */
export function packingView(data: Data, filter = "all") {
  const total = data.items.length;
  const packed = data.items.filter((item) => item.packed).length;
  const categories = data.categories.map((category) => {
    const items = data.items.filter((item) => item.key === category.key);
    return { category, items, total: items.length, packed: items.filter((item) => item.packed).length };
  });
  const sections = categories
    .filter((row) => filter === "all" || filter === "remaining" || row.category.key === filter)
    .map((row) => ({ ...row, items: filter === "remaining" ? row.items.filter((item) => !item.packed) : row.items }));
  const hasItems = sections.some((section) => section.items.length > 0);
  const scopeName = data.categories.find((category) => category.key === filter)?.name ?? "this bag";
  const empty = hasItems ? null
    : total === 0 ? { title: "The tag is empty.", body: "Add a passport, charger, or whatever this trip needs." }
    : filter === "remaining" ? { title: "Everything is packed.", body: "Reset the tag for the next trip, or archive packed items." }
    : { title: `Nothing in ${scopeName} yet.`, body: "Add an item to this category, or switch back to All." };
  return {
    total,
    packed,
    remaining: total - packed,
    percent: total > 0 ? Math.round((packed / total) * 100) : 0,
    complete: total > 0 && packed === total,
    categories,
    sections,
    empty,
  };
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function addItem(tx: Edit, data: Data, input: { text: string; category: string; quantity: number }) {
  const text = input.text.trim();
  if (!text) refuse("Enter an item.");
  if (!data.categories.some((category) => category.key === input.category)) refuse("Choose a bag category.");
  if (!Number.isInteger(input.quantity) || input.quantity < 1 || input.quantity > 9) refuse("Choose a quantity from 1 to 9.");
  return tx.fields.items.insert({ key: input.category, text, quantity: input.quantity });
}
export function setPacked(tx: Edit, item: RowRef, packed: boolean) {
  tx.fields.items.item(item).packed.set(packed);
}
/** Packs or unpacks every item whose state differs; returns how many changed. */
export function setAllPacked(tx: Edit, data: Data, packed: boolean) {
  const changed = data.items.filter((item) => item.packed !== packed);
  for (const item of changed) tx.fields.items.item(item.$id).packed.set(packed);
  return changed.length;
}
export function setEssential(tx: Edit, item: RowRef, essential: boolean) {
  tx.fields.items.item(item).essential.set(essential);
}
/** Steps the quantity 1 to 9 and wraps back to 1. */
export function cycleQuantity(tx: Edit, item: RowRef, quantity: number) {
  tx.fields.items.item(item).quantity.set(quantity >= 9 ? 1 : quantity + 1);
}
export function removeItem(tx: Edit, item: RowRef) {
  tx.fields.items.remove(item);
}
/** Removes every packed item and returns the number removed. */
export function clearPacked(tx: Edit, data: Data) {
  const packed = data.items.filter((item) => item.packed);
  for (const item of packed) tx.fields.items.remove(item.$id);
  return packed.length;
}
/** Adds a preset's items that the tag does not already list (by text); returns how many were added. */
export function applyPreset(tx: Edit, data: Data, presetId: string) {
  const preset = PRESETS.find((candidate) => candidate.id === presetId);
  if (!preset) refuse("Choose a preset: weekend, international, outdoor or business.");
  const existing = new Set(data.items.map((item) => item.text.toLowerCase()));
  const fresh = preset.items.filter((item) => !existing.has(item.text.toLowerCase()));
  for (const item of fresh) {
    tx.fields.items.insert({
      key: item.key,
      text: item.text,
      quantity: item.quantity,
      essential: Boolean(item.essential),
    });
  }
  return fresh.length;
}
export function setFlag(tx: Edit, flag: string) {
  const next = flag.trim();
  if (!next) refuse("Enter a flag or emoji.");
  tx.fields.flag.set(next);
}
