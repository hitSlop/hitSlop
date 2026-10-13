import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import schema, { milestoneTypes } from "./schema";
import { addMilestone, moveMilestone, removeMilestone } from "./shared";

export default defineCommands(schema, {
  addMilestone: {
    description: "Add a dated milestone for a course and return its stable row ID.",
    args: {
      date: s.string({ description: "The date, as YYYY-MM-DD." }),
      title: s.string(),
      courseCode: s.string({ description: "A course code from the term, or ALL." }),
      type: s.enum(milestoneTypes),
    },
    run: ({ tx, current }, args) => addMilestone(tx, current, args),
  },
  moveMilestone: {
    description: "Change a milestone's date.",
    args: {
      milestone: s.string({ description: "The milestone's $id." }),
      date: s.string({ description: "The new date, as YYYY-MM-DD." }),
    },
    run: ({ tx }, { milestone, date }) => moveMilestone(tx, milestone, date),
  },
  removeMilestone: {
    description: "Remove a milestone from the term.",
    args: { milestone: s.string({ description: "The milestone's $id." }) },
    run: ({ tx }, { milestone }) => removeMilestone(tx, milestone),
  },
});
