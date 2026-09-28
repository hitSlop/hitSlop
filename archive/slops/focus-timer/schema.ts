import { defineDocument, s } from "@hitslop/document";

export const timerKinds = ["focus", "rest"] as const;

export default defineDocument({
  focusMinutes: s.integer({ min: 1, max: 120 }),
  restMinutes: s.integer({ min: 1, max: 60 }),
  history: s.list(s.object({
    startedAt: s.string(),
    kind: s.enum(timerKinds),
    seconds: s.integer({ min: 1 }),
  })),
});
