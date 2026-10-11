import { defineDocument, s } from "hitslop";

export default defineDocument({
  title: s.text({ description: "The meeting title shown in the memo." }),
  date: s.string(),
  time: s.string(),
  attendees: s.list(s.string()),
  agenda: s.list(s.object({ text: s.text(), done: s.boolean({ default: false }) })),
  decisions: s.list(s.object({ text: s.text() })),
  notes: s.text(),
  actions: s.list(s.object({ text: s.text(), owner: s.text(), done: s.boolean({ default: false }) })),
}, { initial: {
  title: "Project Kickoff",
  date: "May 12, 2025",
  time: "10:00 AM · Zoom",
  attendees: ["Maya", "Jordan", "Creative Team"],
  agenda: [
    { text: "Review three creative directions", done: true },
    { text: "Confirm production launch dates" },
    { text: "Set Q3 production budget" },
  ],
  decisions: [
    { text: "Lead with quieter tactile photography" },
    { text: "Move launch date to October 6" },
  ],
  notes: "Aligned on MVP scope. Creative team will explore two typography variants.\nFeedback from product was positive on tactile physical skins.\nNext milestone review scheduled for Friday.",
  actions: [
    { text: "Send revised estimate to client", owner: "Jordan" },
    { text: "Book studio and photographer", owner: "Maya", done: true },
    { text: "Share updated timeline across channels", owner: "Jordan" },
  ],
} });
