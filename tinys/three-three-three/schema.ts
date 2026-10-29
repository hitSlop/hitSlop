import { defineDocument, s } from "@hitslop/tinys";

const task = s.object({
  text: s.text(),
  done: s.boolean({ default: false }),
});

export default defineDocument({
  date: s.string(),
  deepWork: s.object({
    project: s.text(),
    minutes: s.integer({ min: 0, max: 180 }),
    notes: s.text(),
  }),
  shortTasks: s.list(task),
  maintenance: s.list(task),
}, { initial: {
  date: "Friday, Sep 25",
  deepWork: {
    project: "Chapter 4 draft: The Economics of Attention",
    minutes: 90,
    notes: "Focus solely on outlining section 4.2 before touching citations. Zero tabs open.",
  },
  shortTasks: [
    { text: "Call accountant regarding quarterly filing" },
    { text: "Confirm venue contract for October offsite", done: true },
    { text: "Send revised proposal to client" },
  ],
  maintenance: [
    { text: "Inbox down to zero unprocessed emails", done: true },
    { text: "45-minute zone 2 run or walk" },
    { text: "Backup local development databases" },
  ],
} });
