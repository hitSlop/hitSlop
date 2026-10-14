import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import schema, { zones } from "./schema";
import { addTask, moveTask, removeTask, setDone } from "./shared";

export default defineCommands(schema, {
  addTask: {
    description: "Place a new task in the holding pen (or the given square) and return its stable row ID.",
    args: {
      text: s.string({ description: "The task text; surrounding spaces are trimmed." }),
      zone: s.optional(s.enum(zones, { description: "Destination: inbox (default), q1, q2, q3 or q4." })),
    },
    run: ({ tx }, { text, zone }) => addTask(tx, text, zone),
  },
  moveTask: {
    description: "Stamp a task into square q1, q2, q3 or q4 and return whether it moved (false if it was already there).",
    args: {
      id: s.string({ description: "The task's $id." }),
      to: s.enum(zones, { description: "Destination square: q1, q2, q3 or q4. The holding pen is refused." }),
    },
    run: ({ tx, current }, { id, to }) => moveTask(tx, current, id, to),
  },
  setDone: {
    description: "Mark a task done or open again; returns null.",
    args: {
      id: s.string({ description: "The task's $id." }),
      done: s.boolean(),
    },
    run: ({ tx }, { id, done }) => { setDone(tx, id, done); },
  },
  removeTask: {
    description: "Delete a task from the blotter; returns null.",
    args: { id: s.string({ description: "The task's $id." }) },
    run: ({ tx }, { id }) => { removeTask(tx, id); },
  },
});
