import { defineDocument, s, type Value } from "@hitslop/document";

const schema = defineDocument({
  level: s.enum(["easy", "medium", "hard"]),
  status: s.enum(["ready", "playing", "won", "lost"]),
  // One character per cell, row by row. cells: h hidden, r revealed, f flagged. mines: 1 mine, 0 clear.
  // `mines` stays empty until the first reveal so that first move is always safe.
  cells: s.string({ maxLength: 400 }),
  mines: s.string({ maxLength: 400 }),
  seconds: s.integer({ min: 0 }),
  best: s.object({ easy: s.integer({ min: 0 }), medium: s.integer({ min: 0 }), hard: s.integer({ min: 0 }) }),
  wins: s.counter(),
  losses: s.counter(),
});

export type Game = Value<typeof schema.descriptor>;
export default schema;
