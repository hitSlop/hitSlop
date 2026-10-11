import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc from "./schema";
import { addChore, addPerson, assignChore, dealOut, newWeek, removeChore, removePerson, setDone } from "./chores";

export default defineCommands(doc, {
  addPerson: {
    description: "Add a person to the household and return their stable row ID.",
    args: { name: s.string({ description: "The person's name." }) },
    run: ({ tx, current }, { name }) => addPerson(tx, current, name),
  },
  removePerson: {
    description: "Remove a person and unassign their chores.",
    args: { person: s.string({ description: "The person's $id." }) },
    run: ({ tx, current }, { person }) => removePerson(tx, current, person),
  },
  addChore: {
    description: "Add a chore for the lightest-loaded person and return its stable row ID.",
    args: { name: s.string({ description: "The chore's name." }) },
    run: ({ tx, current }, { name }) => addChore(tx, current, name),
  },
  removeChore: {
    description: "Remove a chore by its $id.",
    args: { chore: s.string({ description: "The chore's $id." }) },
    run: ({ tx }, { chore }) => removeChore(tx, chore),
  },
  setDone: {
    description: "Mark a chore done or not done, crediting or refunding its points to the assignee.",
    args: {
      chore: s.string({ description: "The chore's $id." }),
      done: s.boolean({ description: "True when the chore is done." }),
    },
    run: ({ tx, current }, { chore, done }) => setDone(tx, current, chore, done),
  },
  assignChore: {
    description: "Assign a chore to a person, or to nobody when person is omitted.",
    args: {
      chore: s.string({ description: "The chore's $id." }),
      person: s.optional(s.string({ description: "The person's $id." })),
    },
    run: ({ tx, current }, { chore, person }) => assignChore(tx, current, chore, person),
  },
  dealOut: {
    description: "Hand every unassigned chore to whoever carries the fewest points and return how many were dealt.",
    args: {},
    run: ({ tx, current }) => dealOut(tx, current),
  },
  newWeek: {
    description: "Clear done marks, rotate rotating chores to the next person and start the next week.",
    args: {},
    run: ({ tx, current }) => newWeek(tx, current),
  },
});
