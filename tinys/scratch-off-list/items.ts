import { refuse, type RowRef, type Tx, type Value } from "@hitslop/tinys";
import type schema from "./schema";
export type Card = Value<typeof schema.descriptor>;
type Edit = Tx<typeof schema>;

/** The ticket and its export derive the tally and the tiles from the same snapshot. */
export function cardView(data: Card) {
  return {
    items: data.items,
    total: data.items.length,
    revealed: data.items.filter(item => item.revealed).length,
    done: data.items.filter(item => item.done).length,
    title: data.title.trim() || "Summer lucky 12",
  };
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function addItem(tx: Edit, text: string) {
  const trimmed = text.trim();
  if (!trimmed) refuse("Enter something to try.");
  return tx.fields.items.insert({ text: trimmed, revealed: true });
}
export function revealItem(tx: Edit, item: RowRef) {
  tx.fields.items.item(item).revealed.set(true);
}
export function setDone(tx: Edit, item: RowRef, done: boolean) {
  tx.fields.items.item(item).done.set(done);
}
export function removeItem(tx: Edit, item: RowRef) {
  tx.fields.items.remove(item);
}
export function recoverAll(tx: Edit, current: Card) {
  const revealed = current.items.filter(item => item.revealed).length;
  for (const item of current.items) {
    const row = tx.at(item);
    row.revealed.set(false);
    row.done.set(false);
  }
  return revealed;
}
