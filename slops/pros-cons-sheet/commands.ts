import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import schema, { sides } from "./schema";
import { statuses } from "./statuses";
import { addFactor, removeFactor, setStatus, setWeight } from "./balance";

export default defineCommands(schema, {
  addFactor: {
    description: "Add a reason to the pros or the cons and return its stable row ID.",
    args: {
      text: s.string({ description: "The reason's text." }),
      weight: s.integer({ min: 1, max: 5, description: "Importance, 1 minor to 5 decisive." }),
      side: s.enum(sides, { description: "pro for a reason in favor, con for a reason against." }),
    },
    run: ({ tx }, args) => addFactor(tx, args),
  },
  setWeight: {
    description: "Set a reason's importance from 1 to 5 by its $id.",
    args: {
      factor: s.string({ description: "The reason's $id." }),
      weight: s.integer({ min: 1, max: 5, description: "Importance, 1 minor to 5 decisive." }),
    },
    run: ({ tx }, { factor, weight }) => setWeight(tx, factor, weight),
  },
  removeFactor: {
    description: "Remove a reason from the balance by its $id.",
    args: { factor: s.string({ description: "The reason's $id." }) },
    run: ({ tx }, { factor }) => removeFactor(tx, factor),
  },
  setStatus: {
    description: "Set the verdict status of the decision.",
    args: {
      status: s.enum(statuses, { description: "evaluating, leaning_pro, leaning_con, decided_pro or decided_con." }),
    },
    run: ({ tx }, { status }) => setStatus(tx, status),
  },
});
