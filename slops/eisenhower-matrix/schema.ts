import { defineDocument, s } from "hitslop";

export const zones = ["inbox", "q1", "q2", "q3", "q4"] as const;

export default defineDocument({
  title: s.text(),
  date: s.text(),
  tasks: s.list(s.object({
    text: s.text(),
    done: s.boolean({ default: false }),
    zone: s.enum(zones),
  })),
}, { initial: {
  title: "Priority Desk Blotter",
  date: "Wed, Sep 16, 2026",
  tasks: [
    { text: "Submit quarterly tax return", zone: "q1" },
    { text: "Patch payment webhook failure", done: true, zone: "q1" },
    { text: "Draft engineering roadmap for v2", zone: "q2" },
    { text: "Weekly deep-work reading (45 min)", zone: "q2" },
    { text: "Strength training session", done: true, zone: "q2" },
    { text: "Reply to inbound vendor cold emails", zone: "q3" },
    { text: "Reschedule dentist booking", zone: "q3" },
    { text: "Audit abandoned Slack channels", zone: "q4" },
    { text: "Review server disk space alert", zone: "inbox" },
  ],
} });
