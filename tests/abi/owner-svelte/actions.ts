import { refuse, s } from "hitslop";
import doc from "./schema";
// A page command runs in the owner's evaluator, like an agent's call.
export const bump = doc.command({
  description: "Add to the hit counter, record the result and report the total.",
  args: { by: s.integer({ min: 1, max: 10 }) },
  run({ current, tx }, { by }) {
    tx.fields.hits.increment(by);
    tx.fields.lastHit.set(current.hits + by);
    return current.hits + by;
  },
});
// A refusal is a message for the person: nothing changes, and the window shows it.
export const decline = doc.command({
  description: "Refuse with a message for the person.",
  args: {},
  run() {
    refuse("Not today.");
  },
});
