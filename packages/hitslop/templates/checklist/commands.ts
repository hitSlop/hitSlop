import { Type } from "hitslop";
import doc from "./schema";

// The Add button and `slop call My.slop addTask --args '{"text":"Buy coffee"}'`
// run this same action. Return the row ID so the page can focus its new input.
export const addTask = doc.command({
  description: "Add an unfinished task and return its row ID.",
  args: Type.Object({ text: Type.String() }, { additionalProperties: false }),
  run({ tx }, { text }) {
    const trimmed = text.trim();
    if (!trimmed) throw new Error("Enter a task.");
    return tx.fields.tasks.insert({ text: trimmed, done: false });
  },
});
