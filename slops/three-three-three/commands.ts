import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import schema from "./schema";
import { adjustFocus, renameTask, setDeepWork, setTaskDone, toggleHour } from "./plan";

export default defineCommands(schema, {
  adjustFocus: {
    description: "Add or subtract whole minutes of deep work, clamped to 0 to 180; return the new minutes.",
    args: { delta: s.integer({ description: "Minutes to add (negative to subtract), such as 15 or -15." }) },
    run: ({ tx, current }, { delta }) => adjustFocus(tx, current, delta),
  },
  toggleHour: {
    description: "Fill deep-work hour 1, 2 or 3, or step back one hour if that hour is already full; return the new minutes.",
    args: { hour: s.integer({ min: 1, max: 3, description: "The hour block, 1 to 3." }) },
    run: ({ tx, current }, { hour }) => toggleHour(tx, current, hour),
  },
  setTaskDone: {
    description: "Mark one urgent or maintenance task done or not done.",
    args: {
      task: s.string({ description: "The task's $id." }),
      done: s.boolean(),
    },
    run: ({ tx, current }, { task, done }) => setTaskDone(tx, current, task, done),
  },
  renameTask: {
    description: "Replace the text of one urgent or maintenance task.",
    args: {
      task: s.string({ description: "The task's $id." }),
      text: s.string(),
    },
    run: ({ tx, current }, { task, text }) => renameTask(tx, current, task, text),
  },
  setDeepWork: {
    description: "Set the deep-work project name and/or session notes; at least one is required.",
    args: {
      project: s.optional(s.string()),
      notes: s.optional(s.string()),
    },
    run: ({ tx }, args) => setDeepWork(tx, args),
  },
});
