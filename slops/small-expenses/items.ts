import { moneyLabel } from "../_shared/format";
import { refuse, type Tx } from "hitslop";
import type doc from "./schema";
export type Data = typeof doc.current;
type Edit = Tx<typeof doc>;
export type Entry = { merchant: string; amountMinor: number; note?: string };

/** The editor and any capture derive totals and counts from the same snapshot. */
export function expenseView(data: Data) {
  const total = data.items.reduce((sum, row) => sum + row.amountMinor, 0);
  const unsettled = data.items.filter(row => !row.settled).reduce((sum, row) => sum + row.amountMinor, 0);
  return { total, unsettled, count: data.items.length };
}

export function formatMoney(minor: number, currency: string) {
  return moneyLabel(minor / 100, currency);
}

/** Checks what a person typed; the editor shows the message inline. */
export function parseEntry(merchant: string, amount: string, note: string): Entry | { error: string } {
  if (!merchant.trim()) return { error: "Add a merchant." };
  if (!/^\d+(\.\d{1,2})?$/.test(amount.trim())) return { error: "Enter an amount with at most two decimal places." };
  const [whole, fraction = ""] = amount.trim().split(".");
  const amountMinor = Number(whole) * 100 + Number(fraction.padEnd(2, "0"));
  if (!Number.isSafeInteger(amountMinor)) return { error: "Amount is too large." };
  return { merchant: merchant.trim(), amountMinor, note: note.trim() };
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function addItem(tx: Edit, entry: Entry) {
  const merchant = entry.merchant.trim();
  if (!merchant) refuse("Add a merchant.");
  const note = entry.note?.trim();
  return tx.fields.items.insert({ merchant, amountMinor: entry.amountMinor, ...(note ? { note } : {}) });
}
export function settleItem(tx: Edit, id: string) {
  tx.fields.items.item(id).settled.set(true);
}
export function settleItems(tx: Edit, ids: readonly string[]) {
  for (const id of ids) settleItem(tx, id);
  return ids.length;
}
export function moveItem(tx: Edit, id: string, to: { before: string } | { after: string }) {
  tx.fields.items.move(id, to);
}
export function clearNote(tx: Edit, id: string) {
  tx.fields.items.item(id).note.clear();
}
export function removeItem(tx: Edit, id: string) {
  tx.fields.items.remove(id);
}
