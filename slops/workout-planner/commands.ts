import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import schema, { restPresets } from "./schema";
import { addExercise, moveExercise, removeExercise, resetProgress, setRestPreset, toggleSet, updateExercise } from "./workout";

const fields = {
  name: s.string({ description: "The exercise name." }),
  sets: s.integer({ min: 1, max: 12, description: "Target sets." }),
  reps: s.integer({ min: 1, max: 50, description: "Target reps per set." }),
  weight: s.string({ description: "Target load, such as \"185 lbs\". Empty means Bodyweight." }),
};

export default defineCommands(schema, {
  addExercise: {
    description: "Add an exercise to the end of the routine and return its stable row ID.",
    args: fields,
    run: ({ tx }, args) => addExercise(tx, args),
  },
  updateExercise: {
    description: "Change an exercise's name, targets and load; completed sets beyond the new set count are dropped.",
    args: { exercise: s.string({ description: "The exercise's $id." }), ...fields },
    run: ({ tx, current }, { exercise, ...rest }) => updateExercise(tx, current, exercise, rest),
  },
  toggleSet: {
    description: "Mark a set done or not done (index counts from 0) and return true when it is now done.",
    args: {
      exercise: s.string({ description: "The exercise's $id." }),
      index: s.integer({ min: 0, max: 11, description: "The set's position, starting at 0." }),
    },
    run: ({ tx, current }, { exercise, index }) => toggleSet(tx, current, exercise, index),
  },
  moveExercise: {
    description: "Move an exercise one place earlier (-1) or later (1) in the routine.",
    args: {
      exercise: s.string({ description: "The exercise's $id." }),
      step: s.enum(["-1", "1"], { description: "-1 moves it earlier, 1 later." }),
    },
    run: ({ tx, current }, { exercise, step }) => moveExercise(tx, current, exercise, step === "-1" ? -1 : 1),
  },
  removeExercise: {
    description: "Remove an exercise from the routine.",
    args: { exercise: s.string({ description: "The exercise's $id." }) },
    run: ({ tx, current }, { exercise }) => removeExercise(tx, current, exercise),
  },
  resetProgress: {
    description: "Clear every completed set and return the number of exercises reset.",
    args: {},
    run: ({ tx, current }) => resetProgress(tx, current),
  },
  setRestPreset: {
    description: "Set the rest timer length in seconds.",
    args: { seconds: s.enum(restPresets, { description: "Rest length in seconds." }) },
    run: ({ tx }, { seconds }) => setRestPreset(tx, seconds),
  },
});
