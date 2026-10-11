import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc, { lists } from "./schema";
import { addPoint, removePoint, setNextDone, setNextMove } from "./choices";

export default defineCommands(doc, {
  addPoint: {
    description: "Add a point to one of the four lists and return its stable row ID.",
    args: {
      list: s.enum(lists, { description: "Which list: hooks, awayMoves, towardsMoves or helpers." }),
      text: s.string(),
    },
    run: ({ tx }, { list, text }) => addPoint(tx, list, text),
  },
  removePoint: {
    description: "Remove a point from one of the four lists.",
    args: {
      list: s.enum(lists, { description: "Which list: hooks, awayMoves, towardsMoves or helpers." }),
      point: s.string({ description: "The point's $id." }),
    },
    run: ({ tx }, { list, point }) => removePoint(tx, list, point),
  },
  setNextMove: {
    description: "Set the one towards move and when it will happen.",
    args: {
      move: s.string(),
      when: s.string(),
    },
    run: ({ tx }, { move, when }) => setNextMove(tx, move, when),
  },
  setNextDone: {
    description: "Mark the one towards move done or not done.",
    args: { done: s.boolean() },
    run: ({ tx }, { done }) => setNextDone(tx, done),
  },
});
