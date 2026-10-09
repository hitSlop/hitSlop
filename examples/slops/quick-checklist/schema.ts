import { defineDocument, s, type Value } from "hitslop";
export const checklist = defineDocument({
  title: s.text({ description: "The title shown in the window." }),
  tasks: s.list(s.object({
    text: s.text(),
    done: s.boolean({ default: false }),
    archived: s.boolean({ default: false, description: "Filed away from the to-do list." }),
  })),
});
export type Checklist = Value<typeof checklist.descriptor>;
export default checklist;

/** The editor and its captures derive their lists, counts and ordering from the same snapshot. */
export function checklistView(data: Checklist) {
  const visible = data.tasks.filter(task => !task.archived);
  const filed = data.tasks.filter(task => task.archived);
  const finished = visible.filter(task => task.done).length;
  return {
    visible, filed, finished,
    ratio: visible.length ? finished / visible.length * 100 : 0,
    marks: visible.length ? Math.round(3 * finished / visible.length) : 0,
  };
}
