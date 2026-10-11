import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc, { modes, themes } from "./schema";
import { appendContent, setContent, setMode, setTheme, setTitle, toggleTheme } from "./markdown";

export default defineCommands(doc, {
  setTitle: {
    description: "Replace the manuscript title.",
    args: { title: s.string() },
    run: ({ tx }, { title }) => setTitle(tx, title),
  },
  setContent: {
    description: "Replace the whole markdown source.",
    args: { content: s.string() },
    run: ({ tx }, { content }) => setContent(tx, content),
  },
  appendContent: {
    description: "Append markdown to the end of the page on a new line and return the new length.",
    args: { text: s.string() },
    run: ({ tx, current }, { text }) => appendContent(tx, current, text),
  },
  setMode: {
    description: "Show the editor as inplace, split or preview.",
    args: { mode: s.enum(modes) },
    run: ({ tx }, { mode }) => setMode(tx, mode),
  },
  setTheme: {
    description: "Switch between the paper and dark themes.",
    args: { theme: s.enum(themes) },
    run: ({ tx }, { theme }) => setTheme(tx, theme),
  },
  toggleTheme: {
    description: "Flip between paper and dark and return the new theme.",
    args: {},
    run: ({ tx, current }) => toggleTheme(tx, current),
  },
});
