import { defineDocument, refuse, s, type Tx } from "hitslop";
// Fields a write leaves out take their default: text starts empty, done starts false.
const schema = defineDocument({
  title: s.text({ description: "The checklist's title." }),
  tasks: s.list(s.object({ text: s.text(), done: s.boolean({ default: false }) }), { description: "Tasks in their display order." }),
}, { initial: {
    title: "A little room to think",
    tasks: [
      { text: "Make something small", done: true },
      { text: "Try a change from the terminal" },
      { text: "Close this list and open it again" },
    ],
  } });
export default schema;

/** One edit for the composer and for agents: the page runs it with doc.change, the
 * addTask command with its tx. */
export function addTask(tx: Tx<typeof schema>, text: string) {
  const trimmed = text.trim();
  if (!trimmed) refuse("Enter a task.");
  return tx.fields.tasks.insert({ text: trimmed });
}
