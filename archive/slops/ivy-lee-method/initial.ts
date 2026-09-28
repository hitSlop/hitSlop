import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
  date: "Friday, Sep 25",
  tasks: [
    { text: "Finalize quarterly financial model", done: false },
    { text: "Review candidate take-home submission", done: false },
    { text: "Draft customer onboarding email series", done: false },
    { text: "Sync with design team on design tokens", done: true },
    { text: "Clear urgent support escalations", done: false },
    { text: "Plan tomorrow’s 6 priorities at sundown", done: false },
  ],
  notes: "Remember: Single-task only. Never move to the next item until the current one is finished.",
} satisfies Input<typeof schema.fields.node>;
