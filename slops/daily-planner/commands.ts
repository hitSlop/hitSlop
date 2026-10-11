import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc, { KINDS } from "./schema";
import {
  addBlock, editBlock, moveBlock, removeBlock, setDate, setNotes, setPriorityDone, setPriorityText,
} from "./schedule";

const time = (description: string) => s.string({ description });

export default defineCommands(doc, {
  setDate: {
    description: "Show another day, as YYYY-MM-DD.",
    args: { date: s.string() },
    run: ({ tx }, { date }) => setDate(tx, date),
  },
  addBlock: {
    description: "Add a time block and return its stable row ID.",
    args: {
      title: s.string(),
      start: time("Start time, HH:MM from 06:00 on a 15 minute step."),
      end: time("End time, HH:MM; 00:00 means midnight."),
      kind: s.optional(s.enum(KINDS)),
    },
    run: ({ tx }, { title, start, end, kind }) => addBlock(tx, { title, start, end, kind: kind ?? "focus" }),
  },
  editBlock: {
    description: "Change a block's title, times and kind.",
    args: {
      block: s.string({ description: "The block's $id." }),
      title: s.string(),
      start: time("Start time, HH:MM."),
      end: time("End time, HH:MM; 00:00 means midnight."),
      kind: s.enum(KINDS),
    },
    run: ({ tx }, { block, ...fields }) => editBlock(tx, block, fields),
  },
  moveBlock: {
    description: "Reschedule a block to new start and end times, keeping its title and kind.",
    args: {
      block: s.string({ description: "The block's $id." }),
      start: time("Start time, HH:MM."),
      end: time("End time, HH:MM; 00:00 means midnight."),
    },
    run: ({ tx }, { block, start, end }) => moveBlock(tx, block, start, end),
  },
  removeBlock: {
    description: "Remove a time block.",
    args: { block: s.string({ description: "The block's $id." }) },
    run: ({ tx }, { block }) => removeBlock(tx, block),
  },
  setPriority: {
    description: "Complete or reopen a priority, and optionally rewrite its text.",
    args: {
      priority: s.string({ description: "The priority's $id." }),
      done: s.boolean(),
      text: s.optional(s.string()),
    },
    run: ({ tx }, { priority, done, text }) => {
      setPriorityDone(tx, priority, done);
      if (text !== undefined) setPriorityText(tx, priority, text);
    },
  },
  setNotes: {
    description: "Replace the day notes.",
    args: { notes: s.string() },
    run: ({ tx }, { notes }) => setNotes(tx, notes),
  },
});
