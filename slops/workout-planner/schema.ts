import { defineDocument, s } from "hitslop";

export const restPresets = ["60", "90", "120", "180"] as const;

export default defineDocument({
  title: s.text(),
  restPreset: s.enum(restPresets),
  exercises: s.list(s.object({
    name: s.text(),
    sets: s.integer({ min: 1, max: 12 }),
    reps: s.integer({ min: 1, max: 50 }),
    completedSetIndices: s.list(s.integer({ min: 0 })),
    weight: s.string(),
  })),
}, {
  initial: {
    title: "Leg day",
    restPreset: "90",
    exercises: [
      { name: "Barbell back squat", sets: 4, reps: 8, weight: "225 lbs", completedSetIndices: [] },
      { name: "Romanian deadlift", sets: 4, reps: 10, weight: "185 lbs", completedSetIndices: [] },
      { name: "Leg press", sets: 3, reps: 12, weight: "360 lbs", completedSetIndices: [] },
      { name: "Lying leg curl", sets: 3, reps: 12, weight: "90 lbs", completedSetIndices: [] },
      { name: "Standing calf raise", sets: 4, reps: 15, weight: "140 lbs", completedSetIndices: [] },
    ],
  },
});
