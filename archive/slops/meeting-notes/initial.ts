import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
  title: "Project Kickoff",
  date: "May 12, 2025",
  time: "10:00 AM · Zoom",
  attendees: ["Maya", "Jordan", "Creative Team"],
  agenda: [
    { text: "Review three creative directions", done: true },
    { text: "Confirm production launch dates", done: false },
    { text: "Set Q3 production budget", done: false },
  ],
  decisions: [
    { text: "Lead with quieter tactile photography" },
    { text: "Move launch date to October 6" },
  ],
  notes: "Aligned on MVP scope. Creative team will explore two typography variants.\nFeedback from product was positive on tactile physical skins.\nNext milestone review scheduled for Friday.",
  actions: [
    { text: "Send revised estimate to client", owner: "Jordan", done: false },
    { text: "Book studio and photographer", owner: "Maya", done: true },
    { text: "Share updated timeline across channels", owner: "Jordan", done: false },
  ],
} satisfies Input<typeof schema.fields.node>;
