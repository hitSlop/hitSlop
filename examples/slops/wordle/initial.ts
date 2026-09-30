import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
  mode: "daily",
  daily: {
    targetWord: "ENTRY",
    guesses: [],
    status: "playing",
    date: "2026-09-16",
  },
  practice: {
    targetWord: "FLOCK",
    guesses: [],
    status: "playing",
  },
  stats: {
    played: 0,
    won: 0,
    currentStreak: 0,
    maxStreak: 0,
    guessDistribution: { "1": 0, "2": 0, "3": 0, "4": 0, "5": 0, "6": 0 },
  },
} satisfies Input<typeof schema.fields.node>;
