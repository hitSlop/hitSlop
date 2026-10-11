import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc from "./schema";
import { clearSessions, saveSession, setSustain, setThresholds } from "./meter";

// Commands only edit document verbs. Listening needs the microphone and happens in the page.
export default defineCommands(doc, {
  setThresholds: {
    description: "Set the levels (0-100) where the light turns amber and red; red must be at least 5 above amber.",
    args: { amber: s.integer(), red: s.integer() },
    run: ({ tx }, { amber, red }) => setThresholds(tx, amber, red),
  },
  setSustain: {
    description: "Set how many seconds the light must stay red to count as too loud.",
    args: { seconds: s.integer() },
    run: ({ tx }, { seconds }) => setSustain(tx, seconds),
  },
  rename: {
    description: "Change the room name shown on the meter.",
    args: { room: s.string({ minLength: 1 }) },
    run: ({ tx }, { room }) => tx.fields.room.set(room),
  },
  logSession: {
    description: "Add a summary of a finished listening session and return its row ID; keeps the 14 most recent.",
    args: {
      date: s.string({ description: "Local day, YYYY-MM-DD." }),
      seconds: s.integer({ min: 1 }),
      loudSeconds: s.integer({ min: 0 }),
      episodes: s.integer({ min: 0 }),
      peak: s.integer({ min: 0, max: 100 }),
    },
    run: ({ tx, current }, session) => saveSession(tx, current, session),
  },
  clearSessions: {
    description: "Remove every saved session summary and return how many were removed.",
    args: {},
    run: ({ tx, current }) => clearSessions(tx, current),
  },
});
