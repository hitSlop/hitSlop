import { defineDocument, s } from "hitslop";

export default defineDocument({
  date: s.string(),
  notes: s.text(),
  tasks: s.list(s.object({
    text: s.text(),
    done: s.boolean({ default: false }),
  })),
}, { initial: {
  date: "Friday, Sep 25",
  tasks: [
    { text: "Finalize quarterly financial model" },
    { text: "Review candidate take-home submission" },
    { text: "Draft customer onboarding email series" },
    { text: "Sync with design team on design tokens", done: true },
    { text: "Clear urgent support escalations" },
    { text: "Plan tomorrow’s 6 priorities at sundown" },
  ],
  notes: "Remember: Single-task only. Never move to the next item until the current one is finished.",
} });
