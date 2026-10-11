import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc from "./schema";
import { setAction, setDeadline, setDone, setGoal, setTheme } from "./chart";

const themeIndex = s.integer({ min: 0, max: 7, description: "The theme's index: 0 (top left) to 7 (bottom right), skipping the centre." });
const actionIndex = s.integer({ min: 0, max: 7, description: "The action's index within its theme, 0 to 7." });

export default defineCommands(doc, {
  setGoal: {
    description: "Write the one core goal in the centre of the sheet. Returns nothing.",
    args: { text: s.string() },
    run: ({ tx }, { text }) => setGoal(tx, text),
  },
  setTheme: {
    description: "Name one of the eight themes around the goal. Returns nothing.",
    args: { theme: themeIndex, title: s.string() },
    run: ({ tx, current }, { theme, title }) => setTheme(tx, current, theme, title),
  },
  setAction: {
    description: "Write the text of one action under a theme. Returns nothing.",
    args: { theme: themeIndex, action: actionIndex, text: s.string() },
    run: ({ tx, current }, { theme, action, text }) => setAction(tx, current, theme, action, text),
  },
  setDone: {
    description: "Tick or untick one action. Returns the new done state.",
    args: { theme: themeIndex, action: actionIndex, done: s.boolean() },
    run: ({ tx, current }, { theme, action, done }) => setDone(tx, current, theme, action, done),
  },
  setDeadline: {
    description: "Set the target date. Returns nothing.",
    args: { deadline: s.string({ description: "A date as YYYY-MM-DD, or an empty string to clear it." }) },
    run: ({ tx }, { deadline }) => setDeadline(tx, deadline),
  },
});
