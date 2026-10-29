import { defineDocument, s } from "@hitslop/tinys";

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

export default defineDocument({
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
}, { initial: {
  mode: "daily",
  daily: { targetWord: "ENTRY", guesses: [], status: "playing", date: "2026-09-16" },
  practice: { targetWord: "FLOCK", guesses: [], status: "playing" },
  stats: {
    played: 0,
    won: 0,
    currentStreak: 0,
    maxStreak: 0,
    guessDistribution: { "1": 0, "2": 0, "3": 0, "4": 0, "5": 0, "6": 0 },
  },
} });
