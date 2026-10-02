import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
  title: "Leg day",
  restPreset: "90",
  exercises: [
    { name: "Barbell back squat", sets: 4, reps: 8, weight: "225 lbs", completedSetIndices: [] },
    { name: "Romanian deadlift", sets: 4, reps: 10, weight: "185 lbs", completedSetIndices: [] },
    { name: "Leg press", sets: 3, reps: 12, weight: "360 lbs", completedSetIndices: [] },
    { name: "Lying leg curl", sets: 3, reps: 12, weight: "90 lbs", completedSetIndices: [] },
    { name: "Standing calf raise", sets: 4, reps: 15, weight: "140 lbs", completedSetIndices: [] },
  ],
} satisfies Input<typeof schema.descriptor>;
