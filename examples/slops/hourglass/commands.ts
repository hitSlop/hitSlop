import { refuse, s } from "hitslop";
import doc from "./schema";

export const startUntil = doc.command({
  description: "Start counting down to an epoch millisecond timestamp in the future.",
  args: { end: s.number({ min: 0 }) },
  run({ tx, now }, { end }) {
    if (end <= now) refuse("Pick a time that hasn't passed.");
    tx.fields.start.set(now);
    tx.fields.end.set(end);
  },
});
export const startFor = doc.command({
  description: "Start a countdown lasting the given number of milliseconds.",
  args: { duration: s.number({ min: 1 }) },
  run({ tx, now }, { duration }) {
    tx.fields.start.set(now);
    tx.fields.end.set(now + duration);
  },
});
export const restart = doc.command({
  description: "Turn the glass over for the same duration as its previous countdown.",
  args: {},
  run({ current, tx, now }) {
    const duration = current.end - current.start;
    if (duration <= 0) refuse("Choose a countdown first.");
    tx.fields.start.set(now);
    tx.fields.end.set(now + duration);
  },
});
