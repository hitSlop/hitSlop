import { defineCommands } from "@hitslop/tinys/commands";
import { s } from "@hitslop/tinys";
import schema from "./schema";
import { logIntake, resetDay, setTarget } from "./intake";

export default defineCommands(schema, {
  logIntake: {
    description: "Log a drink at the current time (today's total is the sum of the logs) and return the log's stable row ID.",
    args: {
      amount: s.number({ description: "Amount drunk, in the document's unit." }),
      label: s.string({ description: "What it was, such as Glass or Bottle." }),
    },
    run: ({ tx, current, now }, { amount, label }) => logIntake(tx, current, amount, label, new Date(now)),
  },
  setTarget: {
    description: "Set the daily intake target.",
    args: { target: s.number({ description: "Daily goal, greater than zero." }) },
    run: ({ tx }, { target }) => setTarget(tx, target),
  },
  resetDay: {
    description: "Remove every logged drink, which clears today's total; returns the number of drinks removed.",
    args: {},
    run: ({ tx, current }) => resetDay(tx, current),
  },
});
