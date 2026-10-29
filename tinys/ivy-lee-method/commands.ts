import { defineCommands } from "@hitslop/tinys/commands";
import { s } from "@hitslop/tinys";
import schema from "./schema";
import { moveTask, rollover, setDate, setDone, setNotes, setTaskText } from "./tasks";

export default defineCommands(schema, {
  setDone: {
    description: "Mark one of the six tasks finished or unfinished and return nothing.",
    args: {
      task: s.string({ description: "The task's $id." }),
      done: s.boolean(),
    },
    run: ({ tx }, { task, done }) => setDone(tx, task, done),
  },
  setTaskText: {
    description: "Replace the text of one task and return nothing.",
    args: {
      task: s.string({ description: "The task's $id." }),
      text: s.string(),
    },
    run: ({ tx }, { task, text }) => setTaskText(tx, task, text),
  },
  moveTask: {
    description: "Swap one task with its neighbor above or below and return nothing; refused at the ends.",
    args: {
      task: s.string({ description: "The task's $id." }),
      direction: s.enum(["up", "down"] as const),
    },
    run: ({ tx, current }, { task, direction }) => moveTask(tx, current, task, direction),
  },
  rollover: {
    description: "Carry unfinished task text into the earliest slots, clear every done mark, set the date, and return the number carried.",
    args: {
      date: s.string({ description: "The date shown on the new docket, such as Saturday, Sep 26." }),
    },
    run: ({ tx, current }, { date }) => rollover(tx, current, date),
  },
  setDate: {
    description: "Set the date shown on the docket and return nothing.",
    args: { date: s.string() },
    run: ({ tx }, { date }) => setDate(tx, date),
  },
  setNotes: {
    description: "Replace the end-of-day reflections and return nothing.",
    args: { notes: s.string() },
    run: ({ tx }, { notes }) => setNotes(tx, notes),
  },
});
