import { refuse, type Tx } from "hitslop";
import type doc from "./schema";

export type Data = typeof doc.current;
type Edit = Tx<typeof doc>;

export const percentOf = (total: number, target: number) =>
  target > 0 ? Math.min(100, Math.round((total / target) * 100)) : 0;

/** The editor, its icon and its captures derive the same readout from one snapshot. */
export function intakeView(data: Data) {
  // Today's total is the sum of the logged drinks; it is never stored separately.
  const total = data.logs.reduce((sum, log) => sum + log.amount, 0);
  const percent = percentOf(total, data.target);
  return { total, percent, fill: Math.max(4, percent), complete: percent >= 100, count: data.logs.length };
}

export function clockTime(date: Date) {
  return `${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}`;
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function logIntake(tx: Edit, current: Data, amount: number, label: string, now: Date) {
  if (!Number.isFinite(amount) || amount <= 0) refuse("Enter an amount greater than zero.");
  if (!label.trim()) refuse("Enter a label for the drink.");
  const first = current.logs[0];
  return tx.fields.logs.insert(
    { time: clockTime(now), amount, label: label.trim() },
    first ? { before: first.$id } : undefined,
  );
}

export function resetDay(tx: Edit, current: Data) {
  const removed = current.logs.length;
  for (const log of current.logs) tx.fields.logs.remove(log.$id);
  return removed;
}

export function setTarget(tx: Edit, target: number) {
  if (!Number.isFinite(target) || target <= 0) refuse("The daily target must be greater than zero.");
  tx.fields.target.set(target);
}
