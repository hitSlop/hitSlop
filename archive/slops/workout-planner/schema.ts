import { defineDocument, s, type Value } from "@hitslop/document";

export const restPresets = ["60", "90", "120", "180"] as const;
export type RestPreset = (typeof restPresets)[number];

const schema = defineDocument({
  title: s.text(),
  restPreset: s.enum(restPresets),
  exercises: s.list(s.object({
    name: s.text(),
    sets: s.integer({ min: 1, max: 12 }),
    reps: s.integer({ min: 1, max: 50 }),
    completedSets: s.integer({ min: 0, max: 12 }),
    completedSetIndices: s.list(s.integer({ min: 0 })),
    weight: s.string(),
  })),
});

export type WorkoutPlanner = Value<typeof schema.fields.node>;
export type Exercise = WorkoutPlanner["exercises"][number];
export default schema;
