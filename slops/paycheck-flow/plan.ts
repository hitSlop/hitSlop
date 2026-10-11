import { refuse, type Tx } from "hitslop";
import type doc from "./schema";
import { flowTotals } from "./flow";
import { maxBuckets, maxSources, minorScale, money, share, tones, type Currency, type Tone } from "./money";

export type Plan = typeof doc.current;
type Edit = Tx<typeof doc>;

/** The editor and the export derive totals and the verdict from the same snapshot. */
export function planView(data: Plan) {
  const { income, spend, gap } = flowTotals(data);
  return {
    income,
    spend,
    gap,
    over: gap < 0,
    verdict: gap >= 0
      ? `${money(gap, data.currency)} left over (${share(gap, income)} of income)`
      : `Over budget by ${money(-gap, data.currency)}`,
    canAddIncome: data.sources.length < maxSources,
    canAddSpending: data.buckets.length < maxBuckets,
  };
}

function label(name: string, what: string): string {
  const trimmed = name.trim();
  if ([...trimmed].length > 24) refuse(`${what} names are at most 24 characters.`);
  return trimmed;
}
function minor(amount: number): number {
  if (!Number.isSafeInteger(amount) || amount < 0) refuse("Amounts are whole minor units, zero or more.");
  return amount;
}
function locate(current: Plan, id: string): "sources" | "buckets" {
  if (current.sources.some((row) => row.$id === id)) return "sources";
  if (current.buckets.some((row) => row.$id === id)) return "buckets";
  return refuse("No income or spending row has that ID.");
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function setMonth(tx: Edit, month: string) {
  tx.fields.month.set(label(month, "Month"));
}

/** Switches currency and rescales every amount so the numbers people typed stay the same. */
export function setCurrency(tx: Edit, current: Plan, currency: Currency) {
  if (currency === current.currency) return;
  const ratio = minorScale(currency) / minorScale(current.currency);
  for (const row of current.sources) tx.at(row).amountMinor.set(Math.round(row.amountMinor * ratio));
  for (const row of current.buckets) tx.at(row).amountMinor.set(Math.round(row.amountMinor * ratio));
  tx.fields.currency.set(currency);
}

export function addIncome(tx: Edit, current: Plan, name: string, amountMinor = 0) {
  if (current.sources.length >= maxSources) refuse(`A plan holds at most ${maxSources} income rows.`);
  return tx.fields.sources.insert({ name: label(name, "Income"), amountMinor: minor(amountMinor) });
}

export function addSpending(tx: Edit, current: Plan, name: string, amountMinor = 0, tone?: Tone) {
  if (current.buckets.length >= maxBuckets) refuse(`A plan holds at most ${maxBuckets} spending rows.`);
  return tx.fields.buckets.insert({
    name: label(name, "Spending"),
    amountMinor: minor(amountMinor),
    tone: tone ?? tones[current.buckets.length % tones.length]!,
  });
}

export function updateRow(tx: Edit, current: Plan, id: string, change: { name?: string; amountMinor?: number; tone?: Tone }) {
  const list = locate(current, id);
  if (list === "sources") {
    if (change.tone !== undefined) refuse("Income rows are always green; only spending rows take a colour.");
    const row = tx.fields.sources.item(id);
    if (change.name !== undefined) row.name.set(label(change.name, "Income"));
    if (change.amountMinor !== undefined) row.amountMinor.set(minor(change.amountMinor));
  } else {
    const row = tx.fields.buckets.item(id);
    if (change.name !== undefined) row.name.set(label(change.name, "Spending"));
    if (change.amountMinor !== undefined) row.amountMinor.set(minor(change.amountMinor));
    if (change.tone !== undefined) row.tone.set(change.tone);
  }
}

export function removeRow(tx: Edit, current: Plan, id: string) {
  tx.fields[locate(current, id)].remove(id);
}
