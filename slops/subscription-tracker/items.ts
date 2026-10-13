import { refuse, type RowRef, type Tx, type Value } from "hitslop";
import type schema from "./schema";
import type { cadences, categories, currencies } from "./schema";
import { isDate } from "./shared";

export type Tracker = Value<typeof schema.descriptor>;
export type Subscription = Tracker["subscriptions"][number];
export type Cadence = (typeof cadences)[number];
export type Category = (typeof categories)[number];
export type Currency = (typeof currencies)[number];
export type Scope = "active" | "all";
export type Details = {
  name: string;
  amount: number;
  cadence: Cadence;
  nextRenewal: string;
  category: Category;
  note: string;
};
type Edit = Tx<typeof schema>;

export function blankDetails(today: string): Details {
  return { name: "", amount: 0, cadence: "monthly", nextRenewal: today, category: "Entertainment", note: "" };
}

export function monthlyAmount(item: Pick<Subscription, "cadence" | "amount">): number {
  return item.cadence === "annual" ? item.amount / 12 : item.amount;
}

/** The page and the export derive their rows and totals from the same snapshot. */
export function subscriptionView(data: Tracker, scope: Scope = "active") {
  const active = data.subscriptions.filter(item => item.active);
  const pace = active.reduce((total, item) => total + monthlyAmount(item), 0);
  const rows = [...data.subscriptions]
    .filter(item => scope === "all" || item.active)
    .sort((a, b) => Number(b.active) - Number(a.active) || a.nextRenewal.localeCompare(b.nextRenewal) || a.name.localeCompare(b.name));
  return {
    rows,
    activeCount: active.length,
    totalCount: data.subscriptions.length,
    pace,
    yearly: pace * 12,
  };
}

function checkDetails(input: Details): Details {
  const name = input.name.trim();
  if (!name) refuse("Enter a service name.");
  const amount = Math.round(Number(input.amount) * 100) / 100;
  if (!Number.isFinite(amount) || amount <= 0) refuse("Enter an amount above zero.");
  if (!isDate(input.nextRenewal)) refuse("Enter a renewal date.");
  return { ...input, name, amount, note: input.note.trim() };
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function addSubscription(tx: Edit, input: Details) {
  return tx.fields.subscriptions.insert(checkDetails(input));
}
export function updateSubscription(tx: Edit, subscription: RowRef, input: Details) {
  const details = checkDetails(input);
  const row = tx.fields.subscriptions.item(subscription);
  row.name.set(details.name);
  row.amount.set(details.amount);
  row.cadence.set(details.cadence);
  row.nextRenewal.set(details.nextRenewal);
  row.category.set(details.category);
  row.note.set(details.note);
}
export function setActive(tx: Edit, subscription: RowRef, active: boolean) {
  tx.fields.subscriptions.item(subscription).active.set(active);
}
export function removeSubscription(tx: Edit, subscription: RowRef) {
  tx.fields.subscriptions.remove(subscription);
}
export function setCurrency(tx: Edit, currency: Currency) {
  tx.fields.currency.set(currency);
}
