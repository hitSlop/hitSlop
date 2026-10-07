import { s } from "hitslop";
import doc from "./schema";
// A page command runs in the owner's evaluator, like an agent's call.
export const bump = doc.command({
  description: "Add to the hit counter and report the total.",
  args: { by: s.integer({ min: 1, max: 10 }) },
  run({ current, tx }, { by }) {
    tx.fields.hits.increment(by);
    return current.hits + by;
  },
});
