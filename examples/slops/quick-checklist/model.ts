import type { Checklist } from "./schema";

/** The editor and captures derive their counts and ordering from the same snapshot. */
export function checklistView(data: Checklist, activeView: "tasks" | "filed") {
  const visible = data.tasks.filter(task => !task.archived);
  const filed = data.tasks.filter(task => task.archived);
  const finished = visible.filter(task => task.done).length;
  const exported = activeView === "filed" ? filed : visible;
  return {
    visible, filed, finished, exported,
    ratio: visible.length ? finished / visible.length * 100 : 0,
    exportFinished: exported.filter(task => task.done).length,
    marks: visible.length ? Math.round(3 * finished / visible.length) : 0,
  };
}
