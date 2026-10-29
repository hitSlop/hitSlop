import { defineCommands } from "@hitslop/tinys/commands";
import { s } from "@hitslop/tinys";
import schema, { colors } from "./schema";
import { addBlock, moveBlock, removeBlock, setBlockDone } from "./schedule";

const dayArg = s.string({ description: "The day's name as shown on the board, such as MON." });
const timeArg = s.string({ description: "Start as HH:MM on a 24-hour clock, or an empty string for an anytime block." });

export default defineCommands(schema, {
  addBlock: {
    description: "Add a time block to a day and return its stable row ID.",
    args: {
      day: dayArg,
      title: s.string(),
      time: timeArg,
      durationMinutes: s.optional(s.integer({ min: 15, max: 1440, description: "Length in minutes; required when time is set." })),
      color: s.enum(colors),
    },
    run: ({ tx, current }, { day, title, time, durationMinutes, color }) =>
      addBlock(tx, current, { day, title, time, durationMinutes, color }),
  },
  moveBlock: {
    description: "Move a block to a day and start time (empty for anytime), optionally resizing it, and return its row ID.",
    args: {
      task: s.string({ description: "The block's $id." }),
      day: dayArg,
      time: timeArg,
      durationMinutes: s.optional(s.integer({ min: 15, max: 1440 })),
    },
    run: ({ tx, current }, { task, day, time, durationMinutes }) =>
      moveBlock(tx, current, task, day, time, durationMinutes),
  },
  setBlockDone: {
    description: "Mark a block done or not done.",
    args: { task: s.string({ description: "The block's $id." }), done: s.boolean() },
    run: ({ tx, current }, { task, done }) => setBlockDone(tx, current, task, done),
  },
  removeBlock: {
    description: "Remove a block from the week.",
    args: { task: s.string({ description: "The block's $id." }) },
    run: ({ tx, current }, { task }) => removeBlock(tx, current, task),
  },
});
