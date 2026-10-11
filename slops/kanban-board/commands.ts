import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc from "./schema";
import { addCard, addLane, laneKeyFrom, moveCard, removeCard, setDoneLane, setLaneLimit } from "./model";

const laneKey = s.string({ description: "The lane's laneKey." });
const card = s.string({ description: "The work order's $id." });

export default defineCommands(doc, {
  addCard: {
    description: "Punch a work order onto the end of a lane and return its stable row ID.",
    args: {
      laneKey,
      title: s.string({ description: "The job title." }),
      note: s.optional(s.string({ description: "Setup, parts, or hold-ups." })),
      tag: s.optional(s.string({ description: "A short tag such as shop or qa." })),
    },
    run: ({ tx, current }, args) => addCard(tx, current, {
      laneKey: args.laneKey,
      title: args.title,
      note: args.note ?? "",
      tag: args.tag ?? "",
    }),
  },
  moveCard: {
    description: "Move a work order to the end of a lane.",
    args: { card, laneKey },
    run: ({ tx, current }, args) => moveCard(tx, current, args.card, args.laneKey),
  },
  removeCard: {
    description: "Remove a work order from the board.",
    args: { card },
    run: ({ tx }, args) => removeCard(tx, args.card),
  },
  addLane: {
    description: "Add an empty lane at the right of the board and return its stable row ID.",
    args: { title: s.string({ description: "The lane's name." }) },
    run: ({ tx, current, random }, args) => addLane(tx, current, { title: args.title, laneKey: laneKeyFrom(random()) }),
  },
  setLaneLimit: {
    description: "Set a lane's work-order limit, or remove it when omitted; returns nothing.",
    args: {
      laneKey,
      limit: s.optional(s.integer({ min: 0, max: 999, description: "Maximum work orders; 0 means no limit." })),
    },
    run: ({ tx, current }, args) => setLaneLimit(tx, current, args.laneKey, args.limit),
  },
  setDoneLane: {
    description: "Make a lane the completed lane, or clear it with an empty laneKey.",
    args: { laneKey: s.string({ description: "The lane's laneKey, or an empty string to clear." }) },
    run: ({ tx, current }, args) => setDoneLane(tx, current, args.laneKey),
  },
});
