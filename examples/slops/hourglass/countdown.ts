import { refuse, type Tx } from "hitslop";
import type doc from "./schema";
export function startUntil(tx: Tx<typeof doc>, end: number, now: number) {
  if (end <= now) refuse("Pick a time that hasn't passed.");
  tx.fields.start.set(now);
  tx.fields.end.set(end);
}
export function restart(tx: Tx<typeof doc>, current: typeof doc.current, now: number) {
  const duration = current.end - current.start;
  if (duration <= 0) refuse("Choose a countdown first.");
  startUntil(tx, now + duration, now);
}
