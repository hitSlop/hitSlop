import { Type } from "hitslop";
import doc from "./schema";

export const startUntil = doc.command({
  description: "Start counting down to an epoch millisecond timestamp in the future.",
  args: Type.Object({ end: Type.Number({ minimum: 0 }) }, { additionalProperties: false }),
  run({ tx, now }, { end }) {
    if (end <= now) throw new Error("Pick a time that hasn't passed.");
    tx.fields.start.set(now);
    tx.fields.end.set(end);
  },
});
export const startFor = doc.command({
  description: "Start a countdown lasting the given number of milliseconds.",
  args: Type.Object({ duration: Type.Number({ minimum: 1 }) }, { additionalProperties: false }),
  run({ tx, now }, { duration }) {
    tx.fields.start.set(now);
    tx.fields.end.set(now + duration);
  },
});
export const restart = doc.command({
  description: "Turn the glass over for the same duration as its previous countdown.",
  args: Type.Object({}, { additionalProperties: false }),
  run({ current, tx, now }) {
    const duration = current.end - current.start;
    if (duration <= 0) throw new Error("Choose a countdown first.");
    tx.fields.start.set(now);
    tx.fields.end.set(now + duration);
  },
});
