import { refuse, s } from "hitslop";
import doc from "./schema";

// The Add button and `slop call My.slop addTask --args '{"text":"Buy coffee"}'`
// run this same action. `refuse` shows its message to the person and changes nothing.
export const addTask = doc.command({
  description: "Add an unfinished task and return its row ID.",
  args: { text: s.string() },
  run({ tx }, { text }) {
    const trimmed = text.trim();
    if (!trimmed) refuse("Enter a task.");
    return tx.fields.tasks.insert({ text: trimmed });
  },
});
