import { defineCommands } from "@hitslop/tinys/commands";
import { s } from "@hitslop/tinys";
import schema from "./schema";
import { clearPeriod, lineKeys, setDate, setWritten, writeLine } from "./journal";

export default defineCommands(schema, {
  setDate: {
    description: "Move the journal to a day and return the date.",
    args: { date: s.string({ description: "The day, YYYY-MM-DD." }) },
    run: ({ tx }, { date }) => { setDate(tx, date); return date; },
  },
  writeLine: {
    description: "Replace one line of the journal (a gratitude, intention, affirmation, highlight, lesson, quote or quote author).",
    args: {
      line: s.enum(lineKeys, { description: "Which line to write." }),
      text: s.string({ description: "The full new text of the line." }),
    },
    run: ({ tx }, { line, text }) => writeLine(tx, line, text),
  },
  markWritten: {
    description: "Mark the morning or evening page as written, or unmark it.",
    args: {
      period: s.enum(["am", "pm"], { description: "am is the morning page, pm the evening page." }),
      done: s.boolean(),
    },
    run: ({ tx }, { period, done }) => setWritten(tx, period, done),
  },
  clearPage: {
    description: "Erase every line of the morning or evening page and unmark it; returns the number of lines cleared.",
    args: { period: s.enum(["am", "pm"], { description: "am is the morning page, pm the evening page." }) },
    run: ({ tx }, { period }) => clearPeriod(tx, period),
  },
});
