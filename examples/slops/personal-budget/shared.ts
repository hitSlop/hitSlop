import { type BudgetCategory } from "./schema";

export function money(value: number): number {
  return Number.isFinite(value) ? value : 0;
}

export function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

export function categoryPercent(cat: BudgetCategory): number {
  const allocated = money(cat.allocated);
  const spent = money(cat.spent);
  if (allocated <= 0) return spent > 0 ? 100 : 0;
  return clamp((spent / allocated) * 100, 0, 100);
}

export function formatCurrency(num: number): string {
  return new Intl.NumberFormat("en-US", { style: "currency", currency: "USD", minimumFractionDigits: 2 }).format(num);
}
