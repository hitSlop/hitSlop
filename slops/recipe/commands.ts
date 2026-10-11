import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc from "./schema";
import { addIngredient, addStep, moveRow, removeRow, setChecked, setStepTimer } from "./recipe";

export default defineCommands(doc, {
  addIngredient: {
    description: "Add an unchecked ingredient to the list and return its stable row ID; blank text is refused.",
    args: { text: s.string({ description: "The ingredient, such as '2 eggs'." }) },
    run: ({ tx }, { text }) => addIngredient(tx, text),
  },
  setChecked: {
    description: "Mark an ingredient gathered or still needed by its $id.",
    args: {
      ingredient: s.string({ description: "The ingredient's $id." }),
      checked: s.boolean({ description: "True when the ingredient is gathered." }),
    },
    run: ({ tx }, { ingredient, checked }) => setChecked(tx, ingredient, checked),
  },
  addStep: {
    description: "Add a method step at the end and return its stable row ID.",
    args: {
      title: s.string({ description: "A short title for the step." }),
      text: s.string({ description: "The instructions for the step." }),
    },
    run: ({ tx }, args) => addStep(tx, args),
  },
  setStepTimer: {
    description: "Set a step's timer length in whole minutes by its $id; 0 means no timer, and omitting minutes clears it.",
    args: {
      step: s.string({ description: "The step's $id." }),
      minutes: s.optional(s.integer({ min: 0, max: 9999, description: "Timer length in minutes." })),
    },
    run: ({ tx }, { step, minutes }) => setStepTimer(tx, step, minutes),
  },
  moveRow: {
    description: "Move an ingredient or step one place up or down by its $id and return nothing.",
    args: {
      kind: s.enum(["ingredient", "step"] as const, { description: "Which list the row is in." }),
      row: s.string({ description: "The row's $id." }),
      direction: s.enum(["up", "down"] as const, { description: "Where to move the row." }),
    },
    run: ({ tx, current }, { kind, row, direction }) => moveRow(tx, current, kind, row, direction),
  },
  removeRow: {
    description: "Remove an ingredient or step by its $id.",
    args: {
      kind: s.enum(["ingredient", "step"] as const, { description: "Which list the row is in." }),
      row: s.string({ description: "The row's $id." }),
    },
    run: ({ tx }, { kind, row }) => removeRow(tx, kind, row),
  },
});
