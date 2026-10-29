import { defineCommands } from "@hitslop/tinys/commands";
import { s } from "@hitslop/tinys";
import schema from "./schema";
import { addDays, dateKey, openPage, writePage } from "./model";

export default defineCommands(schema, {
  writePage: {
    description: "Replace the open page's text, stamping it when it reaches 750 words; returns the word count.",
    args: { text: s.string({ description: "The full text of the open page." }) },
    run: ({ tx, current, now }, { text }) => writePage(tx, current, text, new Date(now)),
  },
  openDay: {
    description: "Open the page for a date, creating it if needed; returns the date as YYYY-MM-DD.",
    args: { date: s.string({ description: "The page's date, YYYY-MM-DD." }) },
    run: ({ tx, current }, { date }) => openPage(tx, current, date),
  },
  openToday: {
    description: "Open today's page, creating it if needed; returns today's date as YYYY-MM-DD.",
    args: {},
    run: ({ tx, current, now }) => openPage(tx, current, dateKey(new Date(now))),
  },
  shiftDay: {
    description: "Open the page a number of days before (negative) or after (positive) the open one; returns the new date.",
    args: { days: s.integer({ description: "Days to move; -1 is the previous day." }) },
    run: ({ tx, current }, { days }) => openPage(tx, current, addDays(current.currentKey, days)),
  },
});
