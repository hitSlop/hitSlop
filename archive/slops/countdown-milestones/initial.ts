import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
  title: "The big trip",
  createdAt: "2026-09-16",
  targetDate: "2026-12-12",
  targetTime: "09:00",
  milestones: [
    { title: "Book the stay", done: false },
    { title: "Plan a few adventures", done: false },
    { title: "Pack the essentials", done: false },
  ],
} satisfies Input<typeof schema.fields.node>;
