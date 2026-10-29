import { refuse, type RowRef, type Tx, type Value } from "@hitslop/tinys";
import type schema from "./schema";
import type { sides } from "./schema";
import { STATUS_LABELS, type Status } from "./statuses";

export type Balance = Value<typeof schema.descriptor>;
export type Side = (typeof sides)[number];
type Edit = Tx<typeof schema>;

export type Weighted = { weight: number };

export function integerWeight(value: number): number {
  return Number.isFinite(value) ? Math.round(value) : 0;
}

export function clampWeight(value: number): number {
  return Math.max(1, Math.min(5, integerWeight(value) || 3));
}

export function totalWeight(items: readonly Weighted[]): number {
  return items.reduce((sum, item) => sum + clampWeight(item.weight), 0);
}

/** Beam rotation in degrees. Heavier For (left) drops counterclockwise. */
export function beamTilt(proTotal: number, conTotal: number): number {
  const total = proTotal + conTotal;
  if (total === 0) return 0;
  return Math.max(-16, Math.min(16, ((conTotal - proTotal) / total) * 32));
}

/** The page, the export and the icon derive their lists, totals and beam from the same snapshot. */
export function balanceView(data: Balance) {
  const pros = data.factors.filter((item) => item.side === "pro");
  const cons = data.factors.filter((item) => item.side === "con");
  const proTotal = totalWeight(pros);
  const conTotal = totalWeight(cons);
  return {
    pros,
    cons,
    proTotal,
    conTotal,
    tilt: beamTilt(proTotal, conTotal),
    statusLabel: STATUS_LABELS[data.status],
  };
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function addFactor(tx: Edit, input: { text: string; weight: number; side: Side }) {
  const text = input.text.trim();
  if (!text) refuse("Enter a reason.");
  return tx.fields.factors.insert({ text, weight: clampWeight(input.weight), side: input.side });
}
export function setWeight(tx: Edit, factor: RowRef, weight: number) {
  tx.fields.factors.item(factor).weight.set(clampWeight(weight));
}
export function removeFactor(tx: Edit, factor: RowRef) {
  tx.fields.factors.remove(factor);
}
export function setStatus(tx: Edit, status: Status) {
  tx.fields.status.set(status);
}
