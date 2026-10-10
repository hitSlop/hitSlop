import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc from "./schema";
import { addTask, fileFinished, restoreTask } from "./tasks";

export default defineCommands(doc, {
  addTask: {
    description: "Add an unfinished task and return its stable row ID.",
    args: { text: s.string() },
    run: ({ tx }, { text }) => addTask(tx, text),
  },
  archiveFinished: {
    description: "File every completed task and return the number filed.",
    args: {},
    run: ({ tx, current }) => fileFinished(tx, current),
  },
  restoreTask: {
    description: "Return a filed task to the active list, unfinished.",
    args: { task: s.string({ description: "The task's $id." }) },
    run: ({ tx }, { task }) => restoreTask(tx, task),
  },
  removeTask: {
    description: "Remove a task from the checklist.",
    args: { task: s.string({ description: "The task's $id." }) },
    run: ({ tx }, { task }) => tx.fields.tasks.remove(task),
  },
});
