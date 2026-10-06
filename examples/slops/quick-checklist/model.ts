import type { Checklist } from "./schema";

/** The editor and captures derive their lists, counts and ordering from the same snapshot. */
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
