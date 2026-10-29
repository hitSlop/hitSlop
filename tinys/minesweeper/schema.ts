import { defineDocument, s } from "@hitslop/tinys";

export const levels = ["easy", "medium", "hard"] as const;
const cells = "h".repeat(36);

export default defineDocument({
  level: s.enum(levels),
  status: s.enum(["ready", "playing", "won", "lost"]),
  // One character per cell, row by row. cells: h hidden, r revealed, f flagged. mines: 1 mine, 0 clear.
  // `mines` stays empty until the first reveal so that first move is always safe.
  cells: s.string({ maxLength: 400 }),
  mines: s.string({ maxLength: 400 }),
  seconds: s.integer({ min: 0 }),
  best: s.object({ easy: s.integer({ min: 0 }), medium: s.integer({ min: 0 }), hard: s.integer({ min: 0 }) }),
  wins: s.counter(),
  losses: s.counter(),
}, { initial: {
  level: "easy",
  status: "ready",
  cells,
  mines: "",
  seconds: 0,
  best: { easy: 0, medium: 0, hard: 0 },
  wins: 0,
  losses: 0,
} });
