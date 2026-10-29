import { defineCommands } from "@hitslop/tinys/commands";
import { s } from "@hitslop/tinys";
import schema from "./schema";
import { cleanRun, logPractice, removeSession, replaceAbc, restartLadder, setLadder, setTempo } from "./practice";

const bpm = (description: string) => s.integer({ min: 20, max: 300, description });

export default defineCommands(schema, {
  setTempo: {
    description: "Set the practice tempo in beats per minute and return it.",
    args: { bpm: bpm("The new tempo.") },
    run: ({ tx }, { bpm }) => setTempo(tx, bpm),
  },
  cleanRun: {
    description: "Record a clean run: raise the tempo one ladder step (never past the goal) and return the new tempo.",
    args: {},
    run: ({ tx, current }) => cleanRun(tx, current),
  },
  restartLadder: {
    description: "Drop the tempo back to the ladder's starting tempo and return it.",
    args: {},
    run: ({ tx, current }) => restartLadder(tx, current),
  },
  setLadder: {
    description: "Change the ladder's starting tempo, goal or step; omitted values stay. Returns the ladder.",
    args: {
      start: s.optional(bpm("The starting tempo.")),
      goal: s.optional(bpm("The goal tempo; must be faster than the start.")),
      step: s.optional(s.integer({ min: 1, max: 40, description: "Beats per minute added after each clean run." })),
    },
    run: ({ tx, current }, change) => setLadder(tx, current, change),
  },
  logPractice: {
    description: "Log a practice session at the current tempo and return its stable row ID; pass the local day, or it defaults to the current UTC date.",
    args: {
      minutes: s.integer({ min: 1, max: 600, description: "Minutes practised." }),
      day: s.optional(s.string({ description: "The day practised, YYYY-MM-DD in the person's local time." })),
    },
    run: ({ tx, current, now }, { minutes, day }) => logPractice(tx, current, day ?? new Date(now).toISOString().slice(0, 10), minutes),
  },
  removeSession: {
    description: "Remove a logged practice session.",
    args: { session: s.string({ description: "The session's $id." }) },
    run: ({ tx }, { session }) => removeSession(tx, session),
  },
  replaceAbc: {
    description: "Replace the piece with new ABC notation (it needs a K: line) and return the tune's title.",
    args: { abc: s.string() },
    run: ({ tx }, { abc }) => replaceAbc(tx, abc),
  },
});
