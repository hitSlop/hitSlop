import { refuse, type Tx, type Value } from "@hitslop/tinys";
import type schema from "./schema";
import { clamp, money } from "./shared";

export type Budget = Value<typeof schema.descriptor>;
type Edit = Tx<typeof schema>;

export const newCategory = { name: "New Category", allocated: 200 } as const;

/** The editor and the export derive totals from the same snapshot. */
export function budgetView(data: Budget) {
  const totalSpent = data.categories.reduce((sum, cat) => sum + money(cat.spent), 0);
  const remaining = data.income - totalSpent;
  return {
    totalSpent,
    remaining,
    remainingRatio: data.income > 0 ? clamp(Math.max(0, remaining) / data.income * 100, 0, 100) : 0,
    overspent: remaining < 0,
  };
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function roundCents(value: number): number {
  return Math.round(value * 100) / 100;
}
function categoryName(name: string): string {
  const label = name.trim();
  if (!label) refuse("Enter a category name.");
  return label;
}
function amount(value: number, message: string): number {
  if (!Number.isFinite(value) || value < 0) refuse(message);
  return value;
}

export function addCategory(tx: Edit, name: string, allocated: number) {
  const label = categoryName(name);
  return tx.fields.categories.insert({ name: label, allocated: amount(allocated, "Allocation must be zero or more."), spent: 0 });
}
export function renameCategory(tx: Edit, id: string, name: string) {
  tx.fields.categories.item(id).name.set(categoryName(name));
}
export function setAllocated(tx: Edit, id: string, allocated: number) {
  tx.fields.categories.item(id).allocated.set(amount(allocated, "Allocation must be zero or more."));
}
export function removeCategory(tx: Edit, id: string) {
  tx.fields.categories.remove(id);
}
/** Adds `value` to a category's spent total and returns the category's name. */
export function logExpense(tx: Edit, current: Budget, id: string, value: number): string {
  const row = current.categories.find(cat => cat.$id === id);
  if (!row) refuse("Select a category first.");
  if (!Number.isFinite(value) || value <= 0) refuse("Enter an amount above zero.");
  tx.fields.categories.item(id).spent.set(roundCents(money(row.spent) + value));
  return row.name;
}
