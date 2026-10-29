import { defineCommands } from "@hitslop/tinys/commands";
import { s } from "@hitslop/tinys";
import schema from "./schema";
import { addStop, cycleStatus, editStop, removeStop, setStatus, setTitle, statuses } from "./board";

export default defineCommands(schema, {
  addStop: {
    description: "Add a stop to the bottom of the board (at most eight) and return its stable row ID.",
    args: {
      time: s.string({ description: "Departure time, up to five characters, such as 09:00." }),
      name: s.string({ description: "Where the stop goes, up to 14 characters; stored upper case." }),
      status: s.optional(s.enum(statuses, { description: "Starting status; on-time when omitted." })),
    },
    run: ({ tx, current }, { time, name, status }) => addStop(tx, current, time, name, status),
  },
  editStop: {
    description: "Change a stop's time, name or both.",
    args: {
      stop: s.string({ description: "The stop's $id." }),
      time: s.optional(s.string({ description: "New departure time, up to five characters." })),
      name: s.optional(s.string({ description: "New destination, up to 14 characters." })),
    },
    run: ({ tx, current }, { stop, time, name }) => editStop(tx, current, stop, { time, text: name }),
  },
  setStatus: {
    description: "Set a stop's status.",
    args: {
      stop: s.string({ description: "The stop's $id." }),
      status: s.enum(statuses),
    },
    run: ({ tx, current }, { stop, status }) => setStatus(tx, current, stop, status),
  },
  cycleStatus: {
    description: "Flip a stop to its next status, as tapping its row does, and return the new status.",
    args: { stop: s.string({ description: "The stop's $id." }) },
    run: ({ tx, current }, { stop }) => cycleStatus(tx, current, stop),
  },
  removeStop: {
    description: "Remove a stop from the board.",
    args: { stop: s.string({ description: "The stop's $id." }) },
    run: ({ tx, current }, { stop }) => removeStop(tx, current, stop),
  },
  setTitle: {
    description: "Set the board's heading, up to 24 characters; stored upper case.",
    args: { title: s.string() },
    run: ({ tx }, { title }) => setTitle(tx, title),
  },
});
