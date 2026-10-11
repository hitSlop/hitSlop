import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc from "./schema";
import { addAssignment, removeAssignment, setCompleted, setDueDate } from "./due";

export default defineCommands(doc, {
  addAssignment: {
    description: "Add an unfinished assignment and return its stable row ID.",
    args: {
      title: s.string({ description: "What needs doing; required." }),
      courseCode: s.string({ description: "A course code such as BIO; may be empty." }),
      dueDate: s.string({ description: "A local YYYY-MM-DD date, or an empty string for undated." }),
      category: s.string({ description: "Homework, Essay, Lab Report and so on; may be empty." }),
      points: s.number({ min: 0, description: "Points the work is worth." }),
      notes: s.string({ description: "Instructions or a place to start; may be empty." }),
    },
    run: ({ tx }, args) => addAssignment(tx, { ...args, completed: false }),
  },
  setCompleted: {
    description: "Mark an assignment finished or not by its $id.",
    args: {
      assignment: s.string({ description: "The assignment's $id." }),
      completed: s.boolean({ description: "True when the work is done." }),
    },
    run: ({ tx }, { assignment, completed }) => setCompleted(tx, assignment, completed),
  },
  setDueDate: {
    description: "Set an assignment's due date by its $id; an empty string makes it undated.",
    args: {
      assignment: s.string({ description: "The assignment's $id." }),
      dueDate: s.string({ description: "A local YYYY-MM-DD date, or an empty string to clear it." }),
    },
    run: ({ tx, current }, { assignment, dueDate }) => setDueDate(tx, current, assignment, dueDate),
  },
  removeAssignment: {
    description: "Remove an assignment by its $id.",
    args: { assignment: s.string({ description: "The assignment's $id." }) },
    run: ({ tx }, { assignment }) => removeAssignment(tx, assignment),
  },
});
