import { defineCommands } from "@hitslop/tinys/commands";
import { s } from "@hitslop/tinys";
import schema from "./schema";
import { addCourse, addQuest, addSideQuest, hit, setDone } from "./quest";

export default defineCommands(schema, {
  addCourse: {
    description: "Add a course region to the map (at most six) and return its stable row ID.",
    args: {},
    run: ({ tx, current }) => addCourse(tx, current),
  },
  addQuest: {
    description: "Add a quest or an exam boss to a course and return its stable row ID.",
    args: {
      course: s.string({ description: "The course's $id." }),
      kind: s.enum(["quest", "boss"], { description: "A plain quest or an exam boss." }),
      due: s.string({ description: "The due date as local YYYY-MM-DD." }),
      title: s.optional(s.string({ description: "The quest title; a default is used when empty." })),
    },
    run: ({ tx }, { course, kind, due, title }) => addQuest(tx, course, kind, due, title ?? ""),
  },
  setQuestDone: {
    description: "Mark a plain quest cleared (earning a sticker) or reopen it; bosses are refused.",
    args: {
      quest: s.string({ description: "The quest's $id." }),
      done: s.boolean({ description: "True to clear the quest, false to reopen it." }),
    },
    run: ({ tx, current }, { quest, done }) => setDone(tx, current, quest, done),
  },
  logStudySession: {
    description: "Log a study session that knocks one HP off an exam boss, or undo one; returns the HP left.",
    args: {
      quest: s.string({ description: "The boss quest's $id." }),
      undo: s.optional(s.boolean({ description: "True to take the last session back instead." })),
    },
    run: ({ tx, current }, { quest, undo }) => hit(tx, current, quest, undo ? -1 : 1),
  },
  addSideQuest: {
    description: "Add an unfinished side quest and return its stable row ID.",
    args: { title: s.string() },
    run: ({ tx }, { title }) => addSideQuest(tx, title),
  },
});
