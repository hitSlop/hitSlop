import { type Cadence, type Category, type Subscription } from "./schema";

export type Draft = {
  name: string;
  amount: number;
  cadence: Cadence;
  nextRenewal: string;
  category: Category;
  note: string;
  active: boolean;
};

export function localDate(date = new Date()): string {
  return new Date(date.getTime() - date.getTimezoneOffset() * 60_000).toISOString().slice(0, 10);
}

export function blankSubscription(): Draft {
  return { name: "", amount: 0, cadence: "monthly", nextRenewal: localDate(), category: "Entertainment", note: "", active: true };
}

export function monthlyAmount(item: Pick<Subscription, "cadence" | "amount">): number {
  return item.cadence === "annual" ? item.amount / 12 : item.amount;
}

export function dateLabel(value: string): string {
  if (!value || Number.isNaN(new Date(`${value}T12:00:00`).getTime())) return "No date";
  return new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric", year: "numeric" }).format(new Date(`${value}T12:00:00`));
}

export function renewalState(value: string): "overdue" | "soon" | "normal" {
  if (!value || Number.isNaN(new Date(`${value}T12:00:00`).getTime())) return "normal";
  const today = localDate();
  if (value < today) return "overdue";
  const soon = new Date(`${today}T12:00:00`);
  soon.setDate(soon.getDate() + 30);
  return value <= localDate(soon) ? "soon" : "normal";
}

export function cadenceLabel(cadence: string): string {
  return cadence === "annual" ? "yr" : "mo";
}
