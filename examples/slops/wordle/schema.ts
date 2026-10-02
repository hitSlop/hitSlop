import { defineDocument, s, type Value } from "@hitslop/document";

export const modes = ["daily", "practice"] as const;
export const statuses = ["playing", "won", "lost"] as const;

function puzzle() {
  return s.object({
    targetWord: s.string(),
    guesses: s.list(s.string()),
    status: s.enum(statuses),
    date: s.optional(s.string()),
  });
}

const schema = defineDocument({
  mode: s.enum(modes),
  daily: puzzle(),
  practice: puzzle(),
  stats: s.object({
    // Tallies merge: concurrent games each add one.
    played: s.counter(),
    won: s.counter(),
    currentStreak: s.number({ min: 0 }),
    maxStreak: s.number({ min: 0 }),
    guessDistribution: s.record(s.integer({ min: 0 })),
  }),
});

export type Wordle = Value<typeof schema.descriptor>;
export type Puzzle = Wordle["daily"];
export default schema;
