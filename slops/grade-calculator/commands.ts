import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc from "./schema";
import { insertCategory, insertCourse, removeCourse, setFinalCategory, setScore, setTarget } from "./gradebook";

const course = s.string({ description: "The course's $id." });
const category = s.string({ description: "The category's $id." });

export default defineCommands(doc, {
  addCourse: {
    description: "Add a placeholder course with three categories and return its stable row ID.",
    args: {},
    run: ({ tx }) => insertCourse(tx),
  },
  removeCourse: {
    description: "Remove a course and its categories from the gradebook.",
    args: { course },
    run: ({ tx, current }, args) => removeCourse(tx, current, args.course),
  },
  addCategory: {
    description: "Add a 10% category to a course and return its stable row ID.",
    args: { course },
    run: ({ tx, current }, args) => insertCategory(tx, current, args.course),
  },
  setScore: {
    description: "Set a category's score percent, or omit score to clear it so it is left out of the average.",
    args: {
      course,
      category,
      score: s.optional(s.number({ description: "The score percent." })),
    },
    run: ({ tx, current }, args) => setScore(tx, current, args.course, args.category, args.score ?? null),
  },
  setTarget: {
    description: "Set a course's target percent that the final-exam solver aims for.",
    args: { course, target: s.number({ description: "Target percent, for example 90." }) },
    run: ({ tx, current }, args) => setTarget(tx, current, args.course, args.target),
  },
  setFinalCategory: {
    description: "Mark one category as the course's final exam, which the solver reads.",
    args: { course, category },
    run: ({ tx, current }, args) => setFinalCategory(tx, current, args.course, args.category),
  },
});
