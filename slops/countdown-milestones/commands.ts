import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import schema from "./schema";
import { addMilestone, moveMilestone, removeMilestone, setCountdown, setMilestoneDone } from "./countdown";

export default defineCommands(schema, {
  setCountdown: {
    description: "Set the event name, date (YYYY-MM-DD) and local time (HH:MM).",
    args: { title: s.string(), date: s.string(), time: s.string() },
    run: ({ tx }, { title, date, time }) => setCountdown(tx, title, date, time),
  },
  addMilestone: {
    description: "Add an unfinished milestone and return its stable row ID.",
    args: { title: s.string() },
    run: ({ tx, current }, { title }) => addMilestone(tx, current, title),
  },
  setMilestoneDone: {
    description: "Mark a milestone complete or incomplete.",
    args: { milestone: s.string({ description: "The milestone's $id." }), done: s.boolean() },
    run: ({ tx }, { milestone, done }) => setMilestoneDone(tx, milestone, done),
  },
  moveMilestone: {
    description: "Move a milestone one place earlier (up) or later (down) in the list.",
    args: { milestone: s.string({ description: "The milestone's $id." }), direction: s.enum(["up", "down"]) },
    run: ({ tx, current }, { milestone, direction }) =>
      moveMilestone(tx, current, milestone, direction === "up" ? -1 : 1),
  },
  removeMilestone: {
    description: "Remove a milestone from the countdown.",
    args: { milestone: s.string({ description: "The milestone's $id." }) },
    run: ({ tx }, { milestone }) => removeMilestone(tx, milestone),
  },
});
