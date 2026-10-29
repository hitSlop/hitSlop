import { refuse, type Tx, type Value } from "@hitslop/tinys";
import type schema from "./schema";
export function startUntil(tx: Tx<typeof schema>, end: number, now: number) {
  if (end <= now) refuse("Pick a time that hasn't passed.");
  tx.fields.start.set(now);
  tx.fields.end.set(end);
}
export function restart(tx: Tx<typeof schema>, current: Value<typeof schema.descriptor>, now: number) {
  const duration = current.end - current.start;
  if (duration <= 0) refuse("Choose a countdown first.");
  startUntil(tx, now + duration, now);
}
