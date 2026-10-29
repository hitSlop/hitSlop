import { defineDocument, s } from "@hitslop/tinys";

export const LEVEL = { min: 5, max: 100 } as const;
export const SUSTAIN = { min: 1, max: 60 } as const;
export const KEPT_SESSIONS = 14;

const quietMeter = defineDocument({
  room: s.text({ description: "The room or place being listened to." }),
  amber: s.integer({ min: LEVEL.min, max: LEVEL.max, default: 50, description: "Level (0-100) where the light turns amber." }),
  red: s.integer({ min: LEVEL.min, max: LEVEL.max, default: 72, description: "Level (0-100) where the light turns red." }),
  sustain: s.integer({ min: SUSTAIN.min, max: SUSTAIN.max, default: 5, description: "Seconds the light must stay red to count as too loud." }),
  sessions: s.list(s.object({
    date: s.string({ description: "Local day, YYYY-MM-DD." }),
    seconds: s.integer({ min: 0, description: "Time spent listening." }),
    loudSeconds: s.integer({ min: 0, description: "Time spent too loud." }),
    episodes: s.integer({ min: 0, description: "How many times the room stayed red for the sustain time." }),
    peak: s.integer({ min: 0, max: 100, description: "Highest smoothed level, 0-100." }),
  }), { description: "Summaries of finished listening sessions, oldest first. No audio is kept." }),
}, { initial: {
  room: "Study hall",
  amber: 50,
  red: 72,
  sustain: 5,
  sessions: [
    { date: "2026-10-07", seconds: 2700, loudSeconds: 95, episodes: 4, peak: 81 },
    { date: "2026-10-08", seconds: 1800, loudSeconds: 20, episodes: 1, peak: 74 },
    { date: "2026-10-09", seconds: 3300, loudSeconds: 0, episodes: 0, peak: 63 },
  ],
} });
export default quietMeter;
