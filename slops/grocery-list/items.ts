import { refuse, type RowRef, type Tx } from "hitslop";
import type doc from "./schema";
import type { categories } from "./schema";
export type Grocery = typeof doc.current;
export type Category = (typeof categories)[number];
export type Filter = "All" | Category;
type Edit = Tx<typeof doc>;

/** The pad and the export derive their rows and counts from the same snapshot. */
export function groceryView(data: Grocery, filter: Filter = "All") {
  const items = filter === "All" ? data.items : data.items.filter((item) => item.category === filter);
  return {
    items,
    done: data.items.filter((item) => item.done).length,
    total: data.items.length,
    memo: data.stickyNote.trim(),
  };
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function addItem(tx: Edit, text: string, category: Category) {
  const trimmed = text.trim();
  if (!trimmed) refuse("Enter an item.");
  return tx.fields.items.insert({ text: trimmed, category });
}
export function setDone(tx: Edit, item: RowRef, done: boolean) {
  tx.fields.items.item(item).done.set(done);
}
export function removeItem(tx: Edit, item: RowRef) {
  tx.fields.items.remove(item);
}
export function clearDone(tx: Edit, current: Grocery) {
  const done = current.items.filter((item) => item.done);
  for (const item of done) tx.fields.items.remove(item.$id);
  return done.length;
}
