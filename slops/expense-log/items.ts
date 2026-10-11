import { refuse, type Tx } from "hitslop";
import type doc from "./schema";
import { formatAmount, formatDate } from "./shared";

export type Expenses = typeof doc.current;
export type Category = Expenses["items"][number]["category"];
type Edit = Tx<typeof doc>;

/** The editor and the export derive totals and sections from the same snapshot. */
export function expenseView(data: Expenses) {
  const symbol = data.currency.trim() || "$";
  const total = data.items.reduce((sum, item) => sum + Number(item.amount || 0), 0);
  const totals = new Map<string, number>();
  for (const item of data.items) {
    totals.set(item.category, (totals.get(item.category) ?? 0) + Number(item.amount || 0));
  }
  return {
    items: data.items,
    count: data.items.length,
    total,
    totalCode: String(Math.round(total * 100)).padStart(6, "0"),
    categoryTotals: Array.from(totals.entries()).sort((a, b) => b[1] - a[1]),
    money: (amount: number) => formatAmount(amount, symbol),
  };
}

function formatTime(d: Date): string {
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function addItem(tx: Edit, input: { title: string; amount: number; category: Category }, at: Date) {
  const title = input.title.trim();
  if (!title) refuse("Enter an item title.");
  if (Number.isNaN(input.amount) || input.amount <= 0) refuse("Enter an amount above zero.");
  return tx.fields.items.insert({
    title,
    amount: Math.round(input.amount * 100) / 100,
    category: input.category,
    date: formatDate(at),
    time: formatTime(at),
  });
}
export function removeItem(tx: Edit, id: string) {
  tx.fields.items.remove(id);
}
export function setCurrency(tx: Edit, value: string) {
  const next = value.replace(/\s/g, "").slice(0, 3);
  tx.fields.currency.set(next.length > 0 ? next : "$");
}
