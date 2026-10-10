import { defineCommands } from "hitslop/commands";
import { refuse, s } from "hitslop";
import doc from "./schema";

export default defineCommands(doc, {
  bump: {
    description: "Add to the hit counter, record the result and report the total.",
    args: { by: s.integer({ min: 1, max: 10 }) },
    run({ current, tx }, { by }) {
      tx.fields.hits.increment(by);
      tx.fields.lastHit.set(current.hits + by);
      return current.hits + by;
    },
  },
  decline: {
    description: "Refuse with a message for the person.",
    args: {},
    run() {
      refuse("Not today.");
    },
  },
});
