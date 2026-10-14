import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import schema from "./schema";
import { startUntil, restart } from "./countdown";

export default defineCommands(schema, {
  startUntil: {
    description: "Start counting down to an epoch millisecond timestamp in the future.",
    args: { end: s.number({ min: 0 }) },
    run({ tx, now }, { end }) {
      startUntil(tx, end, now);
    },
  },
  startFor: {
    description: "Start a countdown lasting the given number of milliseconds.",
    args: { duration: s.number({ min: 1 }) },
    run({ tx, now }, { duration }) {
      startUntil(tx, now + duration, now);
    },
  },
  restart: {
    description: "Turn the glass over for the same duration as its previous countdown.",
    args: {},
    run({ current, tx, now }) {
      restart(tx, current, now);
    },
  },
});
