import { defineDocument, s } from "hitslop";

export const KINDS = ["focus", "meeting", "break", "personal"] as const;

export default defineDocument({
  date: s.string({ description: "The day shown, as YYYY-MM-DD." }),
  priorities: s.list(s.object({
    text: s.text(),
    done: s.boolean({ default: false }),
  })),
  blocks: s.list(s.object({
    start: s.string({ description: "Start time, HH:MM on a 15 minute step." }),
    end: s.string({ description: "End time, HH:MM; 00:00 means midnight." }),
    title: s.text(),
    kind: s.enum(KINDS, { default: "focus" }),
  })),
  notes: s.text(),
}, { initial: {
    date: "2026-09-16",
    priorities: [
      { text: "Finish the quarterly summary" },
      { text: "Call the framer back", done: true },
      { text: "" },
    ],
    blocks: [
      { start: "07:00", end: "08:00", title: "Reading and coffee", kind: "personal" },
      { start: "09:00", end: "11:00", title: "Quarterly summary — first pass", kind: "focus" },
      { start: "11:00", end: "11:30", title: "Team stand-up", kind: "meeting" },
      { start: "12:30", end: "13:15", title: "Lunch away from the desk", kind: "break" },
      { start: "14:00", end: "14:30", title: "Call the framer", kind: "meeting" },
      { start: "15:00", end: "17:00", title: "Edits and inbox", kind: "focus" },
      { start: "18:30", end: "19:30", title: "Swim", kind: "personal" },
    ],
    notes: "Book the van for Saturday. Ask about the delivery window before confirming.",
  } });
